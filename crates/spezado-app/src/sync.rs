// SPDX-License-Identifier: AGPL-3.0-or-later
//! Synchronisation of books between devices through a relay, end-to-end encrypted (librotenado part 0006).
//!
//! Each set of books that is synchronised has, on this device, a member seed, the address of its relay and how far
//! it has read there. The member list lives on the relay and is checked again here, record by record, before a key
//! is used. Events and originals travel in envelopes; what was sent or received is noted, so that nothing travels
//! twice. A person's own devices join by an invitation; when every device is lost, the recovery code made with the
//! first record adds a new one.

use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use librotenado::{
    BooksId, BooksKey, Envelope, EnvelopeContent, EventSet, Invitation, JoinRequest, Member, MemberKeys, MemberList,
    MemberRecord, MemberRole, RecoveryCode, Rights, from_root, request_signature, to_root,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AppError, Books, Workspace, now_ms};

/// Where the state of synchronising a set of books is kept.
const SYNC: &str = "sync";
/// The service under which seeds are kept in the operating system's key store.
pub(crate) const KEYCHAIN: &str = "spezado";
/// Where the books are listed that this device has asked to join.
const JOINING: &str = "joining";
/// How long an invitation is good for, in seconds.
const INVITATION_LIFETIME: u64 = 24 * 60 * 60;
/// How many envelopes are asked for at once.
const PAGE: usize = 500;
/// The largest response read: a page of envelopes with originals.
const MAX_RESPONSE: u64 = 512 * 1024 * 1024;

/// The state of synchronising one set of books on this device.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncState {
    /// The address of the relay.
    relay: String,
    /// The seed of this device as a member, in hexadecimal – empty when it is in the key store.
    seed: String,
    /// Where the seed is: `keychain` – the operating system's key store –, or absent: here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    seed_in: Option<String>,
    /// The relay's position up to which envelopes were received.
    position: i64,
    /// Whether this device is a member yet – after asking to join, it is not until it was added.
    joined: bool,
    /// Invitations made here that are still open.
    #[serde(default)]
    invited: Vec<Invited>,
    /// What this device may do in the books, as the member list says at the last synchronisation.
    #[serde(default)]
    rights: Option<Rights>,
}

/// An invitation made here: its secret in hexadecimal, when it was made, and what the one it brings may do.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Invited {
    secret: String,
    made: u64,
    rights: Rights,
}

/// What a synchronisation did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Events and originals received.
    pub received: usize,
    /// Events and originals sent.
    pub sent: usize,
    /// Devices added by invitation.
    pub added: Vec<String>,
    /// Whether this device still waits to be added.
    pub waiting: bool,
}

/// A member of synchronised books, as shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMember {
    /// Its identity, to remove it by.
    pub id: String,
    /// What it may do: `read`, `write` or `manage`.
    pub rights: Rights,
    /// What people call it.
    pub name: String,
    /// The device name it writes under; none for the recovery code.
    pub device: Option<String>,
    /// Whether it is the recovery code.
    pub recovery: bool,
    /// Whether it is this device.
    pub me: bool,
}

/// Seconds since 1970-01-01 UTC.
fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

fn failed(message: impl std::fmt::Display) -> AppError {
    AppError::Sync(message.to_string())
}

/// Requests to a relay, signed by a member.
struct Relay<'a> {
    agent: ureq::Agent,
    base: String,
    keys: &'a MemberKeys,
}

impl<'a> Relay<'a> {
    fn new(base: &str, keys: &'a MemberKeys) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        Relay { agent, base: base.trim_end_matches('/').to_owned(), keys }
    }

    /// The path as the relay receives it: the base's own path, if any, and the request's.
    fn signed_path(&self, path: &str) -> String {
        let prefix = self.base.split("://").nth(1).and_then(|rest| rest.find('/').map(|i| &rest[i..])).unwrap_or("");
        format!("{prefix}{path}")
    }

    fn get(&self, path: &str) -> Result<(u16, Vec<u8>), AppError> {
        let signature = request_signature(self.keys, "GET", &self.signed_path(path), now_s());
        let response = self
            .agent
            .get(format!("{}{path}", self.base))
            .header("Authorization", signature)
            .call()
            .map_err(|e| failed(format!("the relay {}: {e}", self.base)))?;
        Self::read(response)
    }

    fn post(&self, path: &str, body: Vec<u8>) -> Result<(u16, Vec<u8>), AppError> {
        let signature = request_signature(self.keys, "POST", &self.signed_path(path), now_s());
        let response = self
            .agent
            .post(format!("{}{path}", self.base))
            .header("Authorization", signature)
            .header("Content-Type", "application/json")
            .send(body)
            .map_err(|e| failed(format!("the relay {}: {e}", self.base)))?;
        Self::read(response)
    }

    fn read(mut response: ureq::http::Response<ureq::Body>) -> Result<(u16, Vec<u8>), AppError> {
        let status = response.status().as_u16();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE)
            .read_to_vec()
            .map_err(|e| failed(format!("the relay's answer: {e}")))?;
        Ok((status, body))
    }

    /// The error of an answer that is not a success.
    fn refused(status: u16, body: &[u8]) -> AppError {
        let message = serde_json::from_slice::<Value>(body)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| String::from_utf8_lossy(body).into_owned());
        failed(format!("the relay refused ({status}): {message}"))
    }

    /// The member list of the books, checked record by record.
    fn members(&self, books: &BooksId) -> Result<MemberList, AppError> {
        let (status, body) = self.get(&format!("/v1/books/{books}/members"))?;
        if status != 200 {
            return Err(Self::refused(status, &body));
        }
        let records: Vec<Value> = serde_json::from_slice(&body).map_err(failed)?;
        let mut list = MemberList::new(books.clone());
        for record in records {
            let record: MemberRecord = from_root(&serde_json::to_vec(&record).map_err(failed)?).map_err(failed)?;
            list.apply(record).map_err(|e| failed(format!("the member list is not valid: {e}")))?;
        }
        Ok(list)
    }

    /// Sends a record; whether it was taken (or the head had moved on).
    fn record(&self, books: &BooksId, record: &MemberRecord) -> Result<bool, AppError> {
        let (status, body) = self.post(&format!("/v1/books/{books}/members"), to_root(record).map_err(failed)?)?;
        match status {
            201 => Ok(true),
            409 => Ok(false),
            _ => Err(Self::refused(status, &body)),
        }
    }

    fn envelope(&self, books: &BooksId, envelope: &Envelope) -> Result<(), AppError> {
        let (status, body) = self.post(&format!("/v1/books/{books}/envelopes"), to_root(envelope).map_err(failed)?)?;
        if status == 200 || status == 201 { Ok(()) } else { Err(Self::refused(status, &body)) }
    }
}

/// An invitation as text: the relay, `#join=`, the books, `.` and the secret in base64url.
fn invitation_text(invitation: &Invitation) -> String {
    use base64::Engine;
    let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(invitation.secret);
    format!("{}#join={}.{secret}", invitation.relay.trim_end_matches('/'), invitation.books)
}

/// Reads an invitation from its text.
pub fn parse_invitation(text: &str) -> Result<Invitation, AppError> {
    use base64::Engine;
    let invalid = || failed("not an invitation of spezado");
    let (relay, rest) = text.trim().split_once("#join=").ok_or_else(invalid)?;
    let (books, secret) = rest.split_once('.').ok_or_else(invalid)?;
    let secret = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(secret).map_err(|_| invalid())?;
    Ok(Invitation {
        relay: relay.to_owned(),
        books: books.parse().map_err(|_| invalid())?,
        secret: secret.try_into().map_err(|_| invalid())?,
    })
}

impl Workspace {
    fn sync_state(&self, books: &BooksId) -> Result<Option<SyncState>, AppError> {
        let Some(json) = self.store.setting(Some(books), SYNC)? else { return Ok(None) };
        serde_json::from_str(&json).map(Some).map_err(failed)
    }

    fn save_sync_state(&self, books: &BooksId, state: &SyncState) -> Result<(), AppError> {
        Ok(self.store.set_setting(Some(books), SYNC, &serde_json::to_string(state).map_err(failed)?)?)
    }

    fn keys_of(&self, books: &BooksId, state: &SyncState) -> Result<MemberKeys, AppError> {
        let seed = match state.seed_in.as_deref() {
            Some("keychain") => keyring::Entry::new(KEYCHAIN, &self.keychain_account(books))
                .and_then(|entry| entry.get_secret())
                .map_err(|e| failed(format!("the key of these books is not in the key store of this device: {e}")))?,
            _ => hex::decode(&state.seed).map_err(failed)?,
        };
        Ok(MemberKeys::from_seed(seed.try_into().map_err(|_| failed("a damaged seed"))?))
    }

    fn keychain_account(&self, books: &BooksId) -> String {
        format!("{books} {}", self.device)
    }

    /// A state for new keys: the seed in the key store if it takes it and gives it back, else here.
    fn new_state(&self, books: &BooksId, relay: &str, keys: &MemberKeys, joined: bool) -> SyncState {
        let seed = keys.seed();
        let stored = self.keychain
            && keyring::Entry::new(KEYCHAIN, &self.keychain_account(books))
                .and_then(|entry| entry.set_secret(&seed).and_then(|()| entry.get_secret()))
                .is_ok_and(|back| back == seed);
        SyncState {
            relay: relay.to_owned(),
            seed: if stored { String::new() } else { hex::encode(seed) },
            seed_in: stored.then(|| "keychain".to_owned()),
            position: 0,
            joined,
            invited: vec![],
            rights: joined.then_some(Rights::Manage),
        }
    }

    /// Where the key of synchronised books is kept on this device: `keychain`, or `workspace`.
    pub fn sync_key_store(&self, books: &BooksId) -> Result<Option<&'static str>, AppError> {
        Ok(self
            .sync_state(books)?
            .map(|s| if s.seed_in.as_deref() == Some("keychain") { "keychain" } else { "workspace" }))
    }

    fn joining(&self) -> Result<Vec<BooksId>, AppError> {
        let Some(json) = self.store.setting(None, JOINING)? else { return Ok(Vec::new()) };
        serde_json::from_str(&json).map_err(failed)
    }

    fn note_joining(&self, books: &BooksId) -> Result<(), AppError> {
        let mut joining = self.joining()?;
        if !joining.contains(books) {
            joining.push(books.clone());
        }
        Ok(self.store.set_setting(None, JOINING, &serde_json::to_string(&joining).map_err(failed)?)?)
    }

    /// The relay the books are synchronised through, if they are.
    pub fn sync_relay(&self, books: &BooksId) -> Result<Option<String>, AppError> {
        Ok(self.sync_state(books)?.map(|s| s.relay))
    }

    /// The books synchronised on this device, and those it has asked to join.
    pub fn synchronised(&self) -> Result<Vec<BooksId>, AppError> {
        let mut all: Vec<BooksId> = Vec::new();
        for books in self.books.keys().cloned().chain(self.joining()?) {
            if !all.contains(&books) && self.sync_state(&books)?.is_some() {
                all.push(books);
            }
        }
        Ok(all)
    }

    /// The member this device is in a set of books.
    fn me(&self, keys: &MemberKeys, name: Option<String>) -> Member {
        Member::device(keys, self.device.clone(), name, Rights::Manage)
    }

    /// Starts synchronising books through a relay: founds their member list – this device and a recovery code –
    /// on the relay and sends everything. Returns the recovery code, which the person keeps on paper.
    pub fn start_sync(&mut self, books: &BooksId, relay: &str, name: Option<&str>) -> Result<RecoveryCode, AppError> {
        self.get(books)?;
        if self.sync_state(books)?.is_some() {
            return Err(failed("these books are synchronised already"));
        }
        // What spezado 0.1 kept on this device only goes into the books, to travel with them.
        self.migrate_invoicing(books)?;
        let keys = MemberKeys::generate();
        let recovery = RecoveryCode::generate();
        let me = self.me(&keys, name.map(str::to_owned));
        let (record, _) = MemberRecord::found(books, &keys, vec![me, Member::recovery(&recovery.keys())]);
        if !Relay::new(relay, &keys).record(books, &record)? {
            return Err(failed("the relay holds these books already"));
        }
        let state = self.new_state(books, relay, &keys, true);
        self.save_sync_state(books, &state)?;
        self.sync(books)?;
        Ok(recovery)
    }

    /// An invitation for another device – the person's own, with `manage`, or someone else's with less –, good for a
    /// day; the device joins with it, and the next synchronisation here adds it with these rights.
    pub fn invite(&self, books: &BooksId, rights: Rights) -> Result<String, AppError> {
        let mut state = self.sync_state(books)?.ok_or_else(|| failed("these books are not synchronised"))?;
        if state.rights.is_some_and(|r| r != Rights::Manage) {
            return Err(failed("only a device that may manage the books invites"));
        }
        let invitation = Invitation::new(&state.relay, books.clone());
        state.invited.retain(|i| now_s().saturating_sub(i.made) < INVITATION_LIFETIME);
        state.invited.push(Invited { secret: hex::encode(invitation.secret), made: now_s(), rights });
        self.save_sync_state(books, &state)?;
        Ok(invitation_text(&invitation))
    }

    /// What this device may do in synchronised books, as last seen; none when they are not synchronised.
    pub fn sync_rights(&self, books: &BooksId) -> Result<Option<Rights>, AppError> {
        Ok(self.sync_state(books)?.and_then(|s| s.rights))
    }

    /// Removes a device or person from the books – by its name, device name or identity: a new key is made, which it
    /// does not get. What it has read, it may have kept (librotenado 0006).
    pub fn remove_member(&mut self, books: &BooksId, who: &str) -> Result<String, AppError> {
        let state = self.sync_state(books)?.ok_or_else(|| failed("these books are not synchronised"))?;
        let keys = self.keys_of(books, &state)?;
        let relay = Relay::new(&state.relay, &keys);
        for _ in 0..3 {
            let list = relay.members(books)?;
            let member = list
                .members()
                .find(|m| {
                    m.id.to_string() == who
                        || m.device.as_ref().is_some_and(|d| d.as_str() == who)
                        || m.name.as_deref() == Some(who)
                })
                .cloned()
                .ok_or_else(|| failed(format!("no member {who:?} in these books")))?;
            if member.id == keys.id() {
                return Err(failed("this device cannot remove itself"));
            }
            let current = Self::current_key(&list, &keys)?;
            let (record, _) = list.remove(&keys, &current, vec![member.id]);
            list.clone().apply(record.clone()).map_err(|e| failed(format!("cannot remove {who}: {e}")))?;
            if relay.record(books, &record)? {
                return Ok(member.name.or(member.device.map(|d| d.to_string())).unwrap_or_default());
            }
        }
        Err(failed("the member list keeps changing – try again"))
    }

    /// Asks to join books with an invitation from another device; the books arrive once that device has added this
    /// one, at a later synchronisation.
    pub fn join(&mut self, invitation: &str, name: Option<&str>) -> Result<BooksId, AppError> {
        let invitation = parse_invitation(invitation)?;
        let books = invitation.books.clone();
        if self.books.contains_key(&books) || self.sync_state(&books)?.is_some() {
            return Err(failed("these books are on this device already"));
        }
        let keys = MemberKeys::generate();
        let request: JoinRequest = invitation.request(self.me(&keys, name.map(str::to_owned)));
        let relay = Relay::new(&invitation.relay, &keys);
        let (status, body) = relay.post(&format!("/v1/books/{books}/requests"), to_root(&request).map_err(failed)?)?;
        if status != 201 {
            return Err(Relay::refused(status, &body));
        }
        let state = self.new_state(&books, &invitation.relay, &keys, false);
        self.save_sync_state(&books, &state)?;
        self.note_joining(&books)?;
        Ok(books)
    }

    /// Brings books back to this device with their recovery code, when no other device is left: the recovery
    /// member adds this device, which then receives everything.
    pub fn recover(
        &mut self,
        relay: &str,
        books: &BooksId,
        code: &RecoveryCode,
        name: Option<&str>,
    ) -> Result<SyncReport, AppError> {
        if self.books.contains_key(books) || self.sync_state(books)?.is_some() {
            return Err(failed("these books are on this device already"));
        }
        let recovery = code.keys();
        let client = Relay::new(relay, &recovery);
        let list = client.members(books)?;
        if list.member(&recovery.id()).is_none_or(|m| m.role != MemberRole::Recovery) {
            return Err(failed("this is not the recovery code of these books"));
        }
        let keys = MemberKeys::generate();
        let me = self.me(&keys, name.map(str::to_owned));
        if list.members().any(|m| m.device.as_ref() == Some(&self.device)) {
            return Err(failed(format!("a device named {} belongs to these books already", self.device)));
        }
        let current = Self::current_key(&list, &recovery)?;
        if !client.record(books, &list.add(&recovery, &current, vec![me]))? {
            return Err(failed("the member list changed meanwhile – try again"));
        }
        let state = self.new_state(books, relay, &keys, true);
        self.save_sync_state(books, &state)?;
        self.note_joining(books)?;
        self.sync(books)
    }

    fn current_key(list: &MemberList, keys: &MemberKeys) -> Result<BooksKey, AppError> {
        let index = list.key_index().ok_or_else(|| failed("the books have no key yet"))?;
        let mut all = list.keys(keys).map_err(failed)?;
        all.remove(&index).ok_or_else(|| failed("no current key for this device"))
    }

    /// The members of synchronised books, as the relay lists them.
    pub fn sync_members(&self, books: &BooksId) -> Result<Vec<SyncMember>, AppError> {
        let state = self.sync_state(books)?.ok_or_else(|| failed("these books are not synchronised"))?;
        let keys = self.keys_of(books, &state)?;
        let list = Relay::new(&state.relay, &keys).members(books)?;
        Ok(list
            .members()
            .map(|m| SyncMember {
                id: m.id.to_string(),
                rights: m.rights,
                name: m.name.clone().or_else(|| m.device.as_ref().map(ToString::to_string)).unwrap_or_default(),
                device: m.device.as_ref().map(ToString::to_string),
                recovery: m.role == MemberRole::Recovery,
                me: m.id == keys.id(),
            })
            .collect())
    }

    /// Synchronises every set of books that is: see [`Workspace::sync`].
    pub fn sync_all(&mut self) -> Result<BTreeMap<BooksId, SyncReport>, AppError> {
        let mut reports = BTreeMap::new();
        for books in self.synchronised()? {
            reports.insert(books.clone(), self.sync(&books)?);
        }
        Ok(reports)
    }

    /// Synchronises books with their relay: reads and checks the member list, adds devices that joined with an
    /// invitation from here, receives what other devices sent and sends what this one wrote.
    pub fn sync(&mut self, books: &BooksId) -> Result<SyncReport, AppError> {
        let mut state = self.sync_state(books)?.ok_or_else(|| failed("these books are not synchronised"))?;
        let keys = self.keys_of(books, &state)?;
        let relay = Relay::new(&state.relay, &keys);
        let mut report = SyncReport::default();
        let mut list = relay.members(books)?;
        if list.member(&keys.id()).is_none() {
            let was_added = list.records().iter().any(|r| r.add.iter().any(|m| m.id == keys.id()));
            if state.joined || was_added {
                return Err(failed("this device was removed from the books"));
            }
            report.waiting = true;
            return Ok(report);
        }
        state.joined = true;
        state.rights = list.member(&keys.id()).map(|m| m.rights);

        // Devices that asked to join with an invitation made here.
        state.invited.retain(|i| now_s().saturating_sub(i.made) < INVITATION_LIFETIME);
        if !state.invited.is_empty() && state.rights == Some(Rights::Manage) {
            report.added = self.add_requested(books, &relay, &keys, &mut list, &mut state)?;
        }

        // What other devices sent.
        let all_keys = list.keys(&keys).map_err(failed)?;
        let mut received = EventSet::new();
        loop {
            let path = format!("/v1/books/{books}/envelopes?after={}&limit={PAGE}", state.position);
            let (status, body) = relay.get(&path)?;
            if status != 200 {
                return Err(Relay::refused(status, &body));
            }
            let page: Value = serde_json::from_slice(&body).map_err(failed)?;
            let items = page.get("envelopes").and_then(Value::as_array).cloned().unwrap_or_default();
            for item in &items {
                let envelope: Envelope =
                    from_root(&serde_json::to_vec(&item["envelope"]).map_err(failed)?).map_err(failed)?;
                state.position = item["position"].as_i64().unwrap_or(state.position);
                if envelope.author == keys.id() {
                    continue; // sent from here
                }
                match envelope.content {
                    EnvelopeContent::Event => {
                        let event = envelope.open_event(&all_keys, &list).map_err(failed)?;
                        let (hash, _) = received.insert(event);
                        self.store.mark_sent(books, "event", &hash.to_string())?;
                    }
                    EnvelopeContent::Original => {
                        let bytes = envelope.open(&all_keys).map_err(failed)?;
                        let hash = self.store.put_original(&bytes)?;
                        self.store.mark_sent(books, "original", &hash.to_string())?;
                    }
                }
                report.received += 1;
            }
            if !page.get("more").and_then(Value::as_bool).unwrap_or(false) || items.is_empty() {
                break;
            }
        }
        if !received.is_empty() {
            self.take_in(books, &received)?;
        }
        self.save_sync_state(books, &state)?;

        // What this device wrote, and the originals of its documents.
        let index = list.key_index().ok_or_else(|| failed("the books have no key yet"))?;
        let key = all_keys.get(&index).ok_or_else(|| failed("no current key for this device"))?;
        if let Some(set) = self.books.get(books).map(Books::events) {
            let sent = self.store.sent(books, "event")?;
            for (hash, event) in set.ordered() {
                if event.time.device != self.device || sent.contains(&hash.to_string()) {
                    continue;
                }
                relay.envelope(books, &Envelope::seal_event(books, index, key, &keys, event))?;
                self.store.mark_sent(books, "event", &hash.to_string())?;
                report.sent += 1;
            }
            let sent = self.store.sent(books, "original")?;
            for (_, document) in self.documents(books)? {
                let hash = document.original.hash;
                if sent.contains(&hash.to_string()) {
                    continue;
                }
                if let Some(bytes) = self.store.original(&hash)? {
                    relay.envelope(
                        books,
                        &Envelope::seal(books, index, key, &keys, EnvelopeContent::Original, &bytes),
                    )?;
                    self.store.mark_sent(books, "original", &hash.to_string())?;
                    report.sent += 1;
                }
            }
        }
        Ok(report)
    }

    /// Adds the devices that asked to join with an open invitation; returns their names.
    fn add_requested(
        &mut self,
        books: &BooksId,
        relay: &Relay<'_>,
        keys: &MemberKeys,
        list: &mut MemberList,
        state: &mut SyncState,
    ) -> Result<Vec<String>, AppError> {
        let (status, body) = relay.get(&format!("/v1/books/{books}/requests"))?;
        if status != 200 {
            return Err(Relay::refused(status, &body));
        }
        let requests: Vec<Value> = serde_json::from_slice(&body).map_err(failed)?;
        let invitations: Vec<(Invitation, Rights)> = state
            .invited
            .iter()
            .filter_map(|invited| {
                let secret = hex::decode(&invited.secret).ok()?.try_into().ok()?;
                Some((Invitation { relay: state.relay.clone(), books: books.clone(), secret }, invited.rights))
            })
            .collect();
        let mut used = Vec::new();
        let mut members: Vec<Member> = Vec::new();
        for request in requests {
            let Ok(request) = from_root::<JoinRequest>(&serde_json::to_vec(&request).map_err(failed)?) else {
                continue;
            };
            let Some((invitation, rights)) = invitations.iter().find(|(i, _)| i.proves(&request)) else { continue };
            // What the device may do is the inviting member's to say.
            let member = Member { rights: *rights, ..request.member };
            let taken = list.member(&member.id).is_some()
                || members.iter().chain(list.members()).any(|m| m.device.is_some() && m.device == member.device);
            if member.role != MemberRole::Device || taken {
                continue;
            }
            used.push(hex::encode(invitation.secret));
            members.push(member);
        }
        if members.is_empty() {
            return Ok(Vec::new());
        }
        let names = members
            .iter()
            .map(|m| m.name.clone().or_else(|| m.device.as_ref().map(ToString::to_string)).unwrap_or_default())
            .collect();
        for _ in 0..3 {
            let current = Self::current_key(list, keys)?;
            if relay.record(books, &list.add(keys, &current, members.clone()))? {
                *list = relay.members(books)?;
                state.invited.retain(|i| !used.contains(&i.secret));
                self.save_sync_state(books, state)?;
                return Ok(names);
            }
            *list = relay.members(books)?;
        }
        Err(failed("the member list keeps changing – try again"))
    }

    /// Takes in events received from other devices.
    fn take_in(&mut self, books: &BooksId, received: &EventSet) -> Result<(), AppError> {
        let rows: Vec<_> = received
            .ordered()
            .iter()
            .map(|(hash, _)| (books.clone(), hash.to_string(), received.canonical(hash).expect("in the set").to_vec()))
            .collect();
        for (_, event) in received.ordered() {
            self.clock.observe(&event.time, now_ms());
        }
        self.store.append(&rows)?;
        let entry = self.books.entry(books.clone()).or_default();
        let mut events = entry.events().clone();
        events.merge(received);
        *entry = Books::from_events(events);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitations_read_back() {
        let invitation =
            Invitation::new("https://relay.example", "0192f1e4-0000-7000-8000-0000000000b1".parse().unwrap());
        let text = invitation_text(&invitation);
        assert!(text.starts_with("https://relay.example#join=0192f1e4-0000-7000-8000-0000000000b1."));
        assert_eq!(parse_invitation(&text).unwrap(), invitation);
        assert!(parse_invitation("https://relay.example").is_err());
    }

    #[test]
    fn the_signed_path_has_the_relays_own_path() {
        let keys = MemberKeys::generate();
        assert_eq!(Relay::new("https://relay.example", &keys).signed_path("/v1/x"), "/v1/x");
        assert_eq!(Relay::new("https://example.org/relay/", &keys).signed_path("/v1/x"), "/relay/v1/x");
    }
}

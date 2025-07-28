//! Action types for RPC calls

use serde::{Deserialize, Serialize};

/// Actions available via RPC
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Action {
    // Null action (0)
    Null = 0,

    // Public API actions (1-15)
    ID = 1,
    Dropoff = 2,
    Pickup = 3,

    // Admin API actions (16+)
    CID = 16,
    GetContact = 17,
    GetContacts = 18,
    AddContact = 19,
    DeleteContact = 20,
    GetChannel = 21,
    GetChannels = 22,
    AddChannel = 23,
    DeleteChannel = 24,
    GetProfile = 25,
    GetProfiles = 26,
    AddProfile = 27,
    DeleteProfile = 28,
    LoadProfile = 29,
    GetPeer = 30,
    GetPeers = 31,
    AddPeer = 32,
    DeletePeer = 33,
    Send = 34,
    SendChannel = 35,
    SendMsg = 36,

    // Additional actions
    FlushOutbox = 40,
    GetChannelPrivKey = 41,
    IsRunning = 42,
    Start = 43,
    Stop = 44,
}

impl From<u8> for Action {
    fn from(value: u8) -> Self {
        match value {
            0 => Action::Null,
            1 => Action::ID,
            2 => Action::Dropoff,
            3 => Action::Pickup,
            16 => Action::CID,
            17 => Action::GetContact,
            18 => Action::GetContacts,
            19 => Action::AddContact,
            20 => Action::DeleteContact,
            21 => Action::GetChannel,
            22 => Action::GetChannels,
            23 => Action::AddChannel,
            24 => Action::DeleteChannel,
            25 => Action::GetProfile,
            26 => Action::GetProfiles,
            27 => Action::AddProfile,
            28 => Action::DeleteProfile,
            29 => Action::LoadProfile,
            30 => Action::GetPeer,
            31 => Action::GetPeers,
            32 => Action::AddPeer,
            33 => Action::DeletePeer,
            34 => Action::Send,
            35 => Action::SendChannel,
            36 => Action::SendMsg,
            40 => Action::FlushOutbox,
            41 => Action::GetChannelPrivKey,
            42 => Action::IsRunning,
            43 => Action::Start,
            44 => Action::Stop,
            _ => Action::Null, // Default fallback
        }
    }
}

impl From<Action> for u8 {
    fn from(action: Action) -> Self {
        action as u8
    }
}

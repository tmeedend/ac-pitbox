//! The Steam account signed in right now, which the lobby and `/JSON` ask for.

/// SteamID64 of account 0 in the individual/public universe: an account id
/// (what the registry holds) becomes a SteamID64 by adding it.
const STEAM_ID64_BASE: u64 = 76_561_197_960_265_728;

fn steam_id_from_account(account: u32) -> Option<u64> {
    (account != 0).then(|| STEAM_ID64_BASE + u64::from(account))
}

/// The SteamID64 of the account signed in to the running Steam client.
///
/// Read from `HKCU\Software\Valve\Steam\ActiveProcess\ActiveUser`, which Steam
/// sets while it runs and resets to 0 when nobody is signed in — measured
/// against the `GUID=` Content Manager writes in `race.ini`, same value. The
/// game needs Steam to play online anyway, so a missing Steam is the answer to
/// give, not a case to work around.
#[cfg(windows)]
pub fn active_steam_id() -> Option<u64> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Valve\Steam\ActiveProcess")
        .ok()?;
    let account: u32 = key.get_value("ActiveUser").ok()?;
    steam_id_from_account(account)
}

#[cfg(not(windows))]
pub fn active_steam_id() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rule: the registry's account id maps to the SteamID64 CM itself sends
    /// (checked on a real account), and 0 means nobody is signed in.
    #[test]
    fn an_account_id_becomes_a_steam_id() {
        assert_eq!(steam_id_from_account(1), Some(76_561_197_960_265_729));
        assert_eq!(steam_id_from_account(0), None, "Steam running, nobody signed in");
    }
}

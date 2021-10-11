//! Implementation of balances.
use alloc::string::String;
use alloc::format;
use casper_contract::{contract_api::storage, unwrap_or_revert::UnwrapOrRevert};
use casper_types::{bytesrepr::ToBytes, URef, U256};

use crate::{
    constants::{BALANCES_KEY_NAME},
    detail,
    error::Error,
    Address,
};

/// Creates a dictionary item key for a dictionary item.
#[inline]
fn make_dictionary_item_key(owner: Address) -> String {
    let preimage = owner.to_bytes().unwrap_or_revert();
    // NOTE: As for now dictionary item keys are limited to 64 characters only. Instead of using
    // hashing (which will effectively hash a hash) we'll use base64. Preimage is about 33 bytes for
    // both Address variants, and approximated base64-encoded length will be 4 * (33 / 3) ~ 44
    // characters.
    // Even if the preimage increased in size we still have extra space but even in case of much
    // larger preimage we can switch to base85 which has ratio of 4:5.
    base64::encode(&preimage)
}
/// Get Balance uref of contract context.
pub(crate) fn get_balances_uref() -> URef {
    detail::get_uref(BALANCES_KEY_NAME)
}
/// Make key for balance of given address.
pub(crate) fn balance_key(token_id: &str, address: Address) -> String {
    format!("balances_{}_{}", token_id, make_dictionary_item_key(address))
}
/// Writes token balance of a specified account into a dictionary.
pub(crate) fn write_balance_to(balances_uref: URef, address: Address, token_id: &str, amount: U256) {
    let dictionary_item_key = balance_key(token_id, address);
    storage::dictionary_put(balances_uref, &dictionary_item_key, amount);
}
/// Read token balance of a specified account into a dictionary.
pub(crate) fn read_balance_from(balance_uref: URef, account: Address, token_id: &str) -> U256 {
    let dictionary_item_key = balance_key(token_id, account);
    storage::dictionary_get(balance_uref, &dictionary_item_key)
        .unwrap_or_revert()
        .unwrap_or_default()
}

/// Transfer tokens from the `sender` to the `recipient`.
///
/// This function should not be used directly by contract's entrypoint as it does not validate the
/// sender.
pub(crate) fn transfer_balance(
    balances_uref: URef,
    from: Address,
    to: Address,
    id: &str,
    amount: U256,
) -> Result<(), Error> {
    if from == to || amount.is_zero() {
        return Ok(());
    }
    let sender_balance = read_balance_from(balances_uref, from, id);
        sender_balance
            .checked_sub(amount)
            .ok_or(Error::InsufficientBalance)?;
    let recipient_balance = read_balance_from(balances_uref, to, id);
        recipient_balance
            .checked_add(amount)
            .ok_or(Error::Overflow)?;
    write_balance_to(balances_uref, from, id, sender_balance);
    write_balance_to(balances_uref, to, id, recipient_balance);

    Ok(())
}
use crate::address::Address;
use iota::transaction::prelude::MessageId;

use getset::Getters;
use once_cell::sync::Lazy;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

/// The balance change event data.
#[derive(Getters)]
#[getset(get = "pub")]
pub struct BalanceEvent {
    /// The associated account identifier.
    account_id: [u8; 32],
    /// The associated address.
    address: Address,
    /// The new balance.
    balance: u64,
}

/// A transaction-related event data.
#[derive(Getters)]
#[getset(get = "pub")]
pub struct TransactionEvent {
    /// The associated account identifier.
    account_id: [u8; 32],
    /// The event transaction hash.
    message_id: MessageId,
}

/// A transaction-related event data.
#[derive(Getters)]
#[getset(get = "pub")]
pub struct TransactionConfirmationChangeEvent {
    /// The associated account identifier.
    account_id: [u8; 32],
    /// The event transaction hash.
    message_id: MessageId,
    /// The confirmed state of the transaction.
    confirmed: bool,
}

struct BalanceEventHandler {
    /// The on event callback.
    on_event: Box<dyn Fn(BalanceEvent) + Send>,
}

#[derive(PartialEq)]
pub(crate) enum TransactionEventType {
    NewTransaction,
    Reattachment,
    Broadcast,
}

struct TransactionEventHandler {
    event_type: TransactionEventType,
    /// The on event callback.
    on_event: Box<dyn Fn(TransactionEvent) + Send>,
}

struct TransactionConfirmationChangeEventHandler {
    /// The on event callback.
    on_event: Box<dyn Fn(TransactionConfirmationChangeEvent) + Send>,
}

type BalanceListeners = Arc<Mutex<Vec<BalanceEventHandler>>>;
type TransactionListeners = Arc<Mutex<Vec<TransactionEventHandler>>>;
type TransactionConfirmationChangeListeners =
    Arc<Mutex<Vec<TransactionConfirmationChangeEventHandler>>>;

/// Gets the balance change listeners array.
fn balance_listeners() -> &'static BalanceListeners {
    static LISTENERS: Lazy<BalanceListeners> = Lazy::new(Default::default);
    &LISTENERS
}

/// Gets the transaction listeners array.
fn transaction_listeners() -> &'static TransactionListeners {
    static LISTENERS: Lazy<TransactionListeners> = Lazy::new(Default::default);
    &LISTENERS
}

/// Gets the transaction confirmation change listeners array.
fn transaction_confirmation_change_listeners() -> &'static TransactionConfirmationChangeListeners {
    static LISTENERS: Lazy<TransactionConfirmationChangeListeners> = Lazy::new(Default::default);
    &LISTENERS
}

/// Listen to balance changes.
pub fn on_balance_change<F: Fn(BalanceEvent) + Send + 'static>(cb: F) {
    let mut l = balance_listeners()
        .lock()
        .expect("Failed to lock balance_listeners: on_balance_change()");
    l.push(BalanceEventHandler {
        on_event: Box::new(cb),
    })
}

/// Emits a balance change event.
pub(crate) fn emit_balance_change(account_id: [u8; 32], address: Address, balance: u64) {
    let listeners = balance_listeners()
        .lock()
        .expect("Failed to lock balance_listeners: emit_balance_change()");
    for listener in listeners.deref() {
        (listener.on_event)(BalanceEvent {
            account_id,
            address: address.clone(),
            balance,
        })
    }
}

/// Emits a transaction-related event.
pub(crate) fn emit_transaction_event(
    event_type: TransactionEventType,
    account_id: [u8; 32],
    message_id: MessageId,
) {
    let listeners = transaction_listeners()
        .lock()
        .expect("Failed to lock balance_listeners: emit_balance_change()");
    for listener in listeners.deref() {
        if listener.event_type == event_type {
            (listener.on_event)(TransactionEvent {
                account_id,
                message_id,
            })
        }
    }
}

/// Emits a confirmation state change event.
pub(crate) fn emit_confirmation_state_change(
    account_id: &[u8; 32],
    message_id: MessageId,
    confirmed: bool,
) {
    let listeners = transaction_confirmation_change_listeners()
        .lock()
        .expect("Failed to lock transaction_confirmation_change_listeners: emit_confirmation_state_change()");
    for listener in listeners.deref() {
        (listener.on_event)(TransactionConfirmationChangeEvent {
            account_id: account_id.clone(),
            message_id: message_id.clone(),
            confirmed,
        })
    }
}

/// Adds a transaction-related event listener.
fn add_transaction_listener<F: Fn(TransactionEvent) + Send + 'static>(
    event_type: TransactionEventType,
    cb: F,
) {
    let mut l = transaction_listeners()
        .lock()
        .expect("Failed to lock transaction_listeners: add_transaction_listener()");
    l.push(TransactionEventHandler {
        event_type,
        on_event: Box::new(cb),
    })
}

/// Listen to new messages.
pub fn on_new_transaction<F: Fn(TransactionEvent) + Send + 'static>(cb: F) {
    add_transaction_listener(TransactionEventType::NewTransaction, cb);
}

/// Listen to transaction confirmation state change.
pub fn on_confirmation_state_change<F: Fn(TransactionConfirmationChangeEvent) + Send + 'static>(
    cb: F,
) {
    let mut l = transaction_confirmation_change_listeners().lock().expect(
        "Failed to lock transaction_confirmation_change_listeners: on_confirmation_state_change()",
    );
    l.push(TransactionConfirmationChangeEventHandler {
        on_event: Box::new(cb),
    })
}

/// Listen to transaction reattachment.
pub fn on_reattachment<F: Fn(TransactionEvent) + Send + 'static>(cb: F) {
    add_transaction_listener(TransactionEventType::Reattachment, cb);
}

/// Listen to transaction broadcast.
pub fn on_broadcast<F: Fn(TransactionEvent) + Send + 'static>(cb: F) {
    add_transaction_listener(TransactionEventType::Broadcast, cb);
}

/// Listen to errors.
pub fn on_error<F: Fn(anyhow::Error)>(cb: F) {}

#[cfg(test)]
mod tests {
    use super::{
        emit_balance_change, emit_confirmation_state_change, emit_transaction_event,
        on_balance_change, on_broadcast, on_confirmation_state_change, on_new_transaction,
        on_reattachment, TransactionEventType,
    };
    use crate::address::{AddressBuilder, IotaAddress};
    use iota::transaction::prelude::{Ed25519Address, MessageId};

    #[test]
    fn balance_events() {
        on_balance_change(|event| {
            assert!(event.account_id == [1; 32]);
            assert!(event.balance == 0);
        });

        emit_balance_change(
            [1; 32],
            AddressBuilder::new()
                .address(IotaAddress::Ed25519(Ed25519Address::new([0; 32])))
                .balance(0)
                .key_index(0)
                .build()
                .expect("failed to build address"),
            0,
        );
    }

    #[test]
    fn on_new_transaction_event() {
        let account_id = [0; 32];
        let message_id = MessageId::new([0; 32]);
        let message_id_clone = message_id.clone();
        on_new_transaction(move |event| {
            assert!(event.account_id == account_id);
            assert!(event.message_id == message_id);
        });

        emit_transaction_event(
            TransactionEventType::NewTransaction,
            account_id,
            message_id_clone,
        );
    }

    #[test]
    fn on_reattachment_event() {
        let account_id = [0; 32];
        let message_id = MessageId::new([0; 32]);
        let message_id_clone = message_id.clone();
        on_reattachment(move |event| {
            assert!(event.account_id == account_id);
            assert!(event.message_id == message_id);
        });

        emit_transaction_event(
            TransactionEventType::Reattachment,
            account_id,
            message_id_clone,
        );
    }

    #[test]
    fn on_broadcast_event() {
        let account_id = [5; 32];
        let message_id = MessageId::new([0; 32]);
        let message_id_clone = message_id.clone();
        on_broadcast(move |event| {
            assert!(event.account_id == account_id);
            assert!(event.message_id == message_id);
        });

        emit_transaction_event(
            TransactionEventType::Broadcast,
            account_id,
            message_id_clone,
        );
    }

    #[test]
    fn on_confirmation_state_change_event() {
        let account_id = [6; 32];
        let message_id = MessageId::new([0; 32]);
        let message_id_clone = message_id.clone();
        let confirmed = true;
        on_confirmation_state_change(move |event| {
            assert!(event.account_id == account_id);
            assert!(event.message_id == message_id);
            assert!(event.confirmed == confirmed);
        });

        emit_confirmation_state_change(&account_id, message_id_clone, confirmed);
    }
}

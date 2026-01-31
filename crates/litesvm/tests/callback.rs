use {
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_address::Address,
    solana_signer::Signer,
    solana_system_interface::instruction::transfer,
    solana_transaction::Transaction,
    std::sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

#[test]
fn callback_fires_on_success() {
    let call_count = Arc::new(AtomicU64::new(0));
    let count_clone = call_count.clone();

    let mut svm = LiteSVM::new();
    svm.set_transaction_callback(move |_tx, result, _svm| {
        assert!(result.is_ok());
        count_clone.fetch_add(1, Ordering::Relaxed);
    });

    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();
    svm.airdrop(&from, 1_000_000).unwrap();

    let ix = transfer(&from, &to, 100);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[ix], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // airdrop also calls send_transaction, so count should be 2
    assert_eq!(call_count.load(Ordering::Relaxed), 2);
}

#[test]
fn callback_fires_on_failure() {
    let saw_error = Arc::new(AtomicBool::new(false));
    let saw_error_clone = saw_error.clone();

    let mut svm = LiteSVM::new();
    svm.set_transaction_callback(move |_tx, result, _svm| {
        if result.is_err() {
            saw_error_clone.store(true, Ordering::Relaxed);
        }
    });

    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Address::new_unique();
    svm.airdrop(&from, 10_000).unwrap();

    // Try to transfer more lamports than available after fees
    let ix = transfer(&from, &to, 1_000_000_000);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[ix], Some(&from)),
        svm.latest_blockhash(),
    );
    let result = svm.send_transaction(tx);
    assert!(result.is_err());
    assert!(saw_error.load(Ordering::Relaxed));
}

#[test]
fn callback_receives_correct_data() {
    let captured_signature = Arc::new(Mutex::new(None));
    let sig_clone = captured_signature.clone();
    let captured_balance = Arc::new(AtomicU64::new(0));
    let bal_clone = captured_balance.clone();
    let to = Address::new_unique();
    let to_for_callback = to;

    let mut svm = LiteSVM::new();
    svm.set_transaction_callback(move |_tx, result, svm| {
        if let Ok(meta) = result {
            *sig_clone.lock().unwrap() = Some(meta.signature);
            if let Some(acc) = svm.get_account(&to_for_callback) {
                bal_clone.store(acc.lamports, Ordering::Relaxed);
            }
        }
    });

    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    svm.airdrop(&from, 1_000_000).unwrap();

    let ix = transfer(&from, &to, 42);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[ix], Some(&from)),
        svm.latest_blockhash(),
    );
    let meta = svm.send_transaction(tx).unwrap();

    assert_eq!(*captured_signature.lock().unwrap(), Some(meta.signature));
    assert_eq!(captured_balance.load(Ordering::Relaxed), 42);
}

#[test]
fn unset_callback_stops_invocations() {
    let call_count = Arc::new(AtomicU64::new(0));
    let count_clone = call_count.clone();

    let mut svm = LiteSVM::new();
    svm.set_transaction_callback(move |_tx, _result, _svm| {
        count_clone.fetch_add(1, Ordering::Relaxed);
    });

    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    svm.airdrop(&from, 1_000_000).unwrap();

    let count_after_airdrop = call_count.load(Ordering::Relaxed);
    assert!(count_after_airdrop > 0);

    svm.unset_transaction_callback();

    let to = Address::new_unique();
    let ix = transfer(&from, &to, 100);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[ix], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    // Count should not have increased after unsetting the callback
    assert_eq!(call_count.load(Ordering::Relaxed), count_after_airdrop);
}

#[test]
fn with_transaction_callback_builder() {
    let call_count = Arc::new(AtomicU64::new(0));
    let count_clone = call_count.clone();

    let mut svm = LiteSVM::new().with_transaction_callback(move |_tx, _result, _svm| {
        count_clone.fetch_add(1, Ordering::Relaxed);
    });

    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    svm.airdrop(&from, 1_000_000).unwrap();

    assert!(call_count.load(Ordering::Relaxed) > 0);
}

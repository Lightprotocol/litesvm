use {
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_native_token::LAMPORTS_PER_SOL,
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_system_interface::instruction::{allocate, create_account, transfer},
    solana_transaction::Transaction,
};

#[test_log::test]
fn system_transfer() {
    let from_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let to = Pubkey::new_unique();

    let mut svm = LiteSVM::new();
    let expected_fee = 5000;
    svm.airdrop(&from, 100 + expected_fee).unwrap();

    let instruction = transfer(&from, &to, 64);
    let tx = Transaction::new(
        &[&from_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    let tx_res = svm.send_transaction(tx);

    let from_account = svm.get_account(&from);
    let to_account = svm.get_account(&to);

    assert!(tx_res.is_ok());
    assert_eq!(from_account.unwrap().lamports, 36);
    assert_eq!(to_account.unwrap().lamports, 64);
}

#[test_log::test]
fn system_create_account() {
    let from_keypair = Keypair::new();
    let new_account = Keypair::new();
    let from = from_keypair.pubkey();

    let mut svm = LiteSVM::new();
    let expected_fee = 5000 * 2; // two signers
    let space = 10;
    let rent_amount = svm.minimum_balance_for_rent_exemption(space);
    let lamports = rent_amount + expected_fee;
    svm.airdrop(&from, lamports).unwrap();

    let instruction = create_account(
        &from,
        &new_account.pubkey(),
        rent_amount,
        space as u64,
        &solana_sdk_ids::system_program::id(),
    );
    let tx = Transaction::new(
        &[&from_keypair, &new_account],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    let account = svm.get_account(&new_account.pubkey()).unwrap();

    assert_eq!(account.lamports, rent_amount);
    assert_eq!(account.data.len(), space);
    assert_eq!(account.owner, solana_sdk_ids::system_program::id());
}

#[test_log::test]
fn system_allocate_account() {
    let from_keypair = Keypair::new();
    let new_account_keypair = Keypair::new();
    let from = from_keypair.pubkey();
    let new_account = new_account_keypair.pubkey();

    let mut svm = LiteSVM::new();
    svm.airdrop(&from, 10 * LAMPORTS_PER_SOL).unwrap();

    let instruction = allocate(&new_account, 10);

    let tx = Transaction::new(
        &[&from_keypair, &new_account_keypair],
        Message::new(&[instruction], Some(&from)),
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();

    assert!(svm.get_account(&new_account).is_none());
}

#[test_log::test]
fn test_get_program_accounts() {
    let mut svm = LiteSVM::new();
    let payer_keypair = Keypair::new();
    let payer = payer_keypair.pubkey();

    svm.airdrop(&payer, 10 * LAMPORTS_PER_SOL).unwrap();

    let baseline_count = svm.get_program_accounts(&solana_sdk_ids::system_program::id()).len();

    // Create 3 system-owned accounts
    let mut system_accounts = vec![];
    for i in 0..3 {
        let account_kp = Keypair::new();
        let space = 10 + i;
        let rent = svm.minimum_balance_for_rent_exemption(space);

        let tx = Transaction::new(
            &[&payer_keypair, &account_kp],
            Message::new(
                &[create_account(
                    &payer,
                    &account_kp.pubkey(),
                    rent,
                    space as u64,
                    &solana_sdk_ids::system_program::id(),
                )],
                Some(&payer),
            ),
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();
        system_accounts.push(account_kp.pubkey());
    }

    // Create 2 custom program-owned accounts
    let custom_program = Pubkey::new_unique();
    let mut custom_accounts = vec![];
    for _ in 0..2 {
        let account_kp = Keypair::new();
        let tx = Transaction::new(
            &[&payer_keypair, &account_kp],
            Message::new(
                &[create_account(
                    &payer,
                    &account_kp.pubkey(),
                    svm.minimum_balance_for_rent_exemption(10),
                    10,
                    &custom_program,
                )],
                Some(&payer),
            ),
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();
        custom_accounts.push(account_kp.pubkey());
    }

    // Verify system program accounts
    let sys_accounts = svm.get_program_accounts(&solana_sdk_ids::system_program::id());
    assert_eq!(sys_accounts.len(), baseline_count + 3);
    for pk in &system_accounts {
        assert!(sys_accounts.iter().any(|(p, acc)| p == pk
            && acc.owner == solana_sdk_ids::system_program::id()));
    }

    // Verify custom program accounts
    let prog_accounts = svm.get_program_accounts(&custom_program);
    assert_eq!(prog_accounts.len(), 2);
    for pk in &custom_accounts {
        assert!(prog_accounts.iter().any(|(p, acc)| p == pk && acc.owner == custom_program));
    }

    // Verify consistency with get_account
    let individual: Vec<_> = prog_accounts
        .iter()
        .map(|(pk, _)| (*pk, svm.get_account(pk).unwrap()))
        .collect();
    assert_eq!(prog_accounts, individual);

    // Verify non-existent program returns empty
    assert_eq!(svm.get_program_accounts(&Pubkey::new_unique()).len(), 0);
}

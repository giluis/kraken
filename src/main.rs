#![feature(iter_intersperse)]

use std::{
    collections::HashMap,
    fs::File,
    io::{stdout, BufRead, BufReader, Write},
};

use tokio::task::JoinHandle;


use model::{Account, ClientId};

mod model; 

type TokioSender<T> = tokio::sync::mpsc::Sender<T>;

#[tokio::main]
async fn main() -> std::io::Result<()>{
    let file_name = std::env::args().nth(1).expect("Please input filename as argument");
    let file = File::open(file_name).unwrap();
    let reader = BufReader::new(file).lines().skip(1);

    let mut handler = AccountHandler::new();

    for l in reader {
        let l = l.unwrap();
        handler.process(l).await;
    }

    let result = handler.await_and_eject().await.to_string();
    let _ = stdout().write(result.as_bytes())?;
    Ok(())
}

fn get_client_id(line: &str) -> ClientId {
    line.split(',').nth(1).unwrap().parse().unwrap()
}

#[derive(Debug)]
struct AccountHandler {
    clients: HashMap<ClientId, (TokioSender<String>, JoinHandle<Account>)>,
}

impl AccountHandler {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    async fn process(&mut self, tx_str: String) {
        let id = get_client_id(&tx_str);
        let entry = self.clients.entry(id).or_insert({
            let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
            (
                sender.clone(),
                tokio::spawn(async move {
                    let mut account = Account::empty();
                    loop {
                        match receiver.recv().await {
                            Some(tx_str) => {
                                let tx = tx_str.parse().unwrap();
                                account.process(tx);
                            }
                            None => break,
                        }
                    }
                    account
                }),
            )
        });
        entry
            .0
            .send(tx_str)
            .await
            .expect("Unexpected error on receiver side");
    }

    async fn await_and_eject(self) -> ClientAccounts {
        let mut result = vec![];
        for (i, (s, handle)) in self.clients {
            drop(s);
            result.push((i, handle.await.unwrap()));
        }
        result.sort_by(|(clientid1, _), (clientid2, _)| clientid1.cmp(clientid2));
        ClientAccounts(result)
    }
}

#[derive(Debug)]
struct ClientAccounts(Vec<(ClientId, Account)>);

impl std::fmt::Display for ClientAccounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client, available, held, total, locked\n")?;
        for (id, account) in &self.0 {
            write!(f, "{}, {}\n", id.0, account)?;
        }
        std::fmt::Result::Ok(())
    }
}

const PRECISION_FACTOR: f64 = 1e4;
fn round_equals(first: f64, second: f64) -> bool {
    (first * PRECISION_FACTOR).round() / PRECISION_FACTOR
        == (second * PRECISION_FACTOR).round() / PRECISION_FACTOR
}

#[cfg(test)]
mod integration_tests {
    use crate::ClientAccounts;

    use super::test_cases::*;
    use std::process::Command;

    impl PartialEq for ClientAccounts {
        fn eq(&self, other: &Self) -> bool {
            self.0.len() == other.0.len()
                && self.0.iter().zip(other.0.iter()).all(
                    |((client_id1, account1), (client_id2, account2))| {
                        client_id1 == client_id2 && account1.are_same_without_transactions(account2)
                    },
                )
        }
    }

    #[test]
    fn run_integration_tests() {
        let test_cases = [
            disputes_are_non_idempotent_after_resolve,
            chargeback_withdraws_and_locks_account,
            disputes_are_idempotent_before_resolve,
            unexistent_dispute_reference,
            unexistent_resolve_reference,
            resolve_only_decs_respective_held,
            resolve_decs_held_incs_available,
            dispute_only_affects_selected_client,
            withdrawal_doesnt_decrement_held,
            deposit_doesnt_increment_held,
            dispute_holds_funds,
            no_withdrawal_of_held_funds,
            withdrawal_fails_when_account_is_empty,
            withdrawal_succeeds_when_same_as_available,
            withdrawal_succeeds_when_less_than_available,
            withdrawal_fails_when_more,
            deposits_accumulate,
        ];

        // increases statistical likelyness
        // of undesired task ordering
        // acceptable substitute for accurate async testing
        for _ in 0..12 {
            for t in test_cases {
                let TestCase {
                    inputs,
                    expected_outputs,
                } = t();
                let mut contents = format!("type, client, tx, amount\n");
                contents += &(inputs
                    .iter()
                    .map(|t| format!("{t}"))
                    .intersperse("\n".to_owned())
                    .collect::<String>());
                std::fs::write("temp.csv", contents).unwrap();
                let result = String::from_utf8(
                    Command::new("cargo")
                        .arg("run")
                        .arg("--")
                        .arg("temp.csv")
                        .output()
                        .unwrap()
                        .stdout,
                )
                .unwrap();
                let actual: ClientAccounts = result.parse().unwrap();
                assert_eq!(actual, expected_outputs);
            }
        }
    }
}

mod test_cases {
    use std::str::FromStr;

    use crate::model::{Account, ClientId, Transaction, TxId, TxType};

    use crate::ClientAccounts;

    pub struct TestCase {
        pub inputs: Vec<Transaction>,
        pub expected_outputs: ClientAccounts,
    }

    fn tx_of_client(txs: &[Transaction], client_id: ClientId) -> Vec<Transaction> {
        txs.iter()
            .filter(|tx| tx.client() == client_id && tx.is_deposit_or_withdrawal())
            .cloned()
            .collect::<Vec<_>>()
    }

    pub trait TransactionExt {
        fn deposit(client_id: ClientId, tx_id: TxId, value: f64) -> Self;

        fn withdrawal(client_id: ClientId, tx_id: TxId, value: f64) -> Self;

        fn dispute(client_id: ClientId, tx_id: TxId) -> Self;

        fn resolve(client_id: ClientId, tx_id: TxId) -> Self;

        fn chargeback(client_id: ClientId, tx_id: TxId) -> Self;
    }

    impl TransactionExt for Transaction {
        fn deposit(client_id: ClientId, tx_id: TxId, value: f64) -> Self {
            Self::new(client_id, tx_id, TxType::Deposit(value))
        }

        fn withdrawal(client_id: ClientId, tx_id: TxId, value: f64) -> Self {
            Self::new(client_id, tx_id, TxType::Withdrawal(value))
        }

        fn dispute(client_id: ClientId, tx_id: TxId) -> Self {
            Self::new(client_id, tx_id, TxType::Dispute)
        }

        fn resolve(client_id: ClientId, tx_id: TxId) -> Self {
            Self::new(client_id, tx_id, TxType::Resolve)
        }

        fn chargeback(client_id: ClientId, tx_id: TxId) -> Self {
            Self::new(client_id, tx_id, TxType::ChargeBack)
        }
    }

    impl FromStr for ClientAccounts {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let mut clients = vec![];
            for l in s.lines().skip(1) {
                let mut iter = l.split(',');
                let client_id = ClientId(iter.next().unwrap().parse().unwrap());
                let available = iter.next().unwrap().trim().parse().unwrap();
                let held = iter.next().unwrap().trim().parse().unwrap();
                let _skip_total = iter.next();
                let is_locked = iter.next().unwrap().trim().parse().unwrap();
                clients.push((client_id, Account::new(available, held, vec![], is_locked)));
            }
            Ok(ClientAccounts(clients))
        }
    }

    // TODO: test disputes, resolves and chargebacks that reference
    // inexistent transactions, or transactions that are neither a deposit nor a withdrawal

    pub fn disputes_are_non_idempotent_after_resolve() -> TestCase {
        let client_id0 = ClientId(0);
        let initial_amount = 10.191;
        let tx_id = TxId(0);
        let inputs = vec![
            Transaction::deposit(client_id0, tx_id, initial_amount),
            Transaction::dispute(client_id0, tx_id),
            Transaction::resolve(client_id0, tx_id),
            Transaction::dispute(client_id0, tx_id),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].set_disputed(true);

        TestCase {
            inputs,
            expected_outputs: ClientAccounts(vec![(
                client_id0,
                Account::new(0.0, initial_amount, client_transactions, false),
            )]),
        }
    }

    pub fn chargeback_withdraws_and_locks_account() -> TestCase {
        let client_id0 = ClientId(0);
        let initial_amount = 10.1423;
        let tx_id = TxId(0);
        let inputs = vec![
            Transaction::deposit(client_id0, tx_id, initial_amount),
            Transaction::dispute(client_id0, tx_id),
            Transaction::chargeback(client_id0, tx_id),
            // should not appear in transaction history
            Transaction::deposit(client_id0, TxId(1), initial_amount),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        // TODO: Document this
        client_transactions[0].set_disputed(true);
        // remove last deposit, has it shouldn't be present in transaction history
        client_transactions.pop();

        let expected_outputs = ClientAccounts(vec![(
            client_id0,
            Account::new(0.0, 0.0, client_transactions, true),
        )]);

        TestCase {
            inputs,
            expected_outputs,
        }
    }

    pub fn disputes_are_idempotent_before_resolve() -> TestCase {
        let client_id0 = ClientId(0);
        let initial_amount = 10.09;
        let tx_id = TxId(0);
        let inputs = vec![
            Transaction::deposit(client_id0, tx_id, initial_amount),
            Transaction::dispute(client_id0, tx_id),
            Transaction::dispute(client_id0, tx_id),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id0,
            Account::unlocked(0.0, initial_amount, client_transactions),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn unexistent_resolve_reference() -> TestCase {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::dispute(client_id0, TxId(0)),
            Transaction::resolve(client_id0, TxId(100)),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn unexistent_dispute_reference() -> TestCase {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::dispute(client_id0, TxId(100)),
        ];

        let expected_outputs = ClientAccounts(vec![(
            client_id0,
            Account::with_available(initial_amount, tx_of_client(&inputs, client_id0)),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn resolve_only_decs_respective_held() -> TestCase {
        let client_id0 = ClientId(0);
        let client_id1 = ClientId(1);
        let disputed_id1 = TxId(3);
        let disputed_id2 = TxId(4);
        let disputed_amount1 = 1.0;
        let disputed_amount2 = 3.0;
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::deposit(client_id1, TxId(1), initial_amount),
            Transaction::deposit(client_id0, disputed_id1, disputed_amount1),
            Transaction::dispute(client_id0, disputed_id1),
            Transaction::deposit(client_id1, disputed_id2, disputed_amount2),
            Transaction::dispute(client_id1, disputed_id2),
            Transaction::resolve(client_id0, disputed_id1),
        ];

        // client 1 dispute has been resolved
        // client 2 dispute has not
        let mut client1_transactions = tx_of_client(&inputs, client_id1);
        client1_transactions[1].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![
            (
                client_id0,
                Account::with_available(
                    initial_amount + disputed_amount1,
                    tx_of_client(&inputs, client_id0),
                ),
            ),
            (
                client_id1,
                Account::unlocked(initial_amount, disputed_amount2, client1_transactions),
            ),
        ]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn resolve_decs_held_incs_available() -> TestCase {
        let client_id = ClientId(0);
        let disputed_id = TxId(1);
        let disputed_amount = 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, disputed_id, disputed_amount),
            Transaction::dispute(client_id, disputed_id),
            Transaction::resolve(client_id, disputed_id),
        ];

        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::with_available(disputed_amount, tx_of_client(&inputs, client_id)),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn dispute_only_affects_selected_client() -> TestCase {
        let client_id0 = ClientId(0);
        let client_id1 = ClientId(1);
        let disputed_id1 = TxId(3);
        let disputed_amount = 1.0;
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::deposit(client_id1, TxId(1), initial_amount),
            Transaction::deposit(client_id0, disputed_id1, disputed_amount),
            Transaction::deposit(client_id1, TxId(1), initial_amount),
            Transaction::dispute(client_id0, disputed_id1),
        ];

        let mut client0_transactions = tx_of_client(&inputs, client_id0);
        client0_transactions[1].set_disputed(true);

        // dispute did not affect client 2
        let expected_outputs = ClientAccounts(vec![
            (
                client_id0,
                Account::unlocked(initial_amount, disputed_amount, client0_transactions),
            ),
            (
                client_id1,
                Account::with_available(initial_amount * 2.0, tx_of_client(&inputs, client_id1)),
            ),
        ]);

        TestCase {
            inputs,
            expected_outputs,
        }
    }

    pub fn withdrawal_doesnt_decrement_held() -> TestCase {
        let client_id = ClientId(0);
        let disputed_id = TxId(1);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let withdrawn = 3.0;
        assert!(withdrawn < initial_amount);
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), initial_amount),
            Transaction::deposit(client_id, disputed_id, disputed_amount),
            Transaction::dispute(client_id, disputed_id),
            Transaction::withdrawal(client_id, TxId(2), withdrawn),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id);
        client_transactions[1].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::unlocked(
                initial_amount - withdrawn,
                disputed_amount,
                client_transactions,
            ),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn deposit_doesnt_increment_held() -> TestCase {
        let client_id = ClientId(0);
        let disputed_id = TxId(1);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let add_amount = 3.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), initial_amount),
            Transaction::deposit(client_id, disputed_id, disputed_amount),
            Transaction::dispute(client_id, disputed_id),
            Transaction::deposit(client_id, TxId(2), add_amount),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id);
        client_transactions[1].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::unlocked(
                initial_amount + add_amount,
                disputed_amount,
                client_transactions,
            ),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn dispute_holds_funds() -> TestCase {
        let client_id = ClientId(0);
        let deposit_id = TxId(1);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), initial_amount),
            Transaction::deposit(client_id, deposit_id, disputed_amount),
            Transaction::dispute(client_id, deposit_id),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id);
        client_transactions[1].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::unlocked(initial_amount, disputed_amount, client_transactions),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn no_withdrawal_of_held_funds() -> TestCase {
        let client_id = ClientId(0);
        let deposit_id = TxId(0);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let withdrawn_amount = 2.0;
        let inputs = vec![
            Transaction::deposit(client_id, deposit_id, initial_amount),
            Transaction::deposit(client_id, deposit_id, disputed_amount),
            Transaction::dispute(client_id, deposit_id),
            Transaction::withdrawal(client_id, TxId(1), 2.0),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id);
        client_transactions[1].set_disputed(true);

        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::unlocked(
                initial_amount - withdrawn_amount,
                disputed_amount,
                client_transactions,
            ),
        )]);

        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn withdrawal_fails_when_account_is_empty() -> TestCase {
        let client_id = ClientId(0);
        let inputs = vec![Transaction::withdrawal(client_id, TxId(1), 1.0)];
        let expected_outputs = ClientAccounts(vec![(
            client_id,
            // not added to history
            Account::with_available(0.0, vec![]),
        )]);
        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn withdrawal_succeeds_when_same_as_available() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), val_1),
        ];
        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::with_available(0.0, inputs.clone()),
        )]);
        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn withdrawal_succeeds_when_less_than_available() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let withdraw = val_1 - 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), withdraw),
        ];
        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::with_available(val_1 - withdraw, tx_of_client(&inputs, client_id)),
        )]);
        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn withdrawal_fails_when_more() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let withdraw = val_1 + 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), withdraw),
        ];
        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::with_available(val_1, vec![inputs[0].clone()]),
        )]);
        TestCase {
            expected_outputs,
            inputs,
        }
    }

    pub fn deposits_accumulate() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let val_2 = val_1 + 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::deposit(client_id, TxId(1), val_2),
            Transaction::deposit(client_id, TxId(2), 0.0),
        ];
        let expected_outputs = ClientAccounts(vec![(
            client_id,
            Account::with_available(val_1 + val_2, tx_of_client(&inputs, client_id)),
        )]);
        TestCase {
            expected_outputs,
            inputs,
        }
    }
}

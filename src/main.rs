#![feature(hash_set_entry)]
use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

// TODO: explore cache-friendlier alternatives
//      perhaps a matrix that covered all the transactions that happnned X seconds ago, where X the typicall amount that disputes occurs
// TODO: dispute, resolve, and cashback tx
//       handle tx_ids differently than deposit and withdrawals
//       perhaps auto generate the required IDs
// TODO: sinalize which transactions have gone wrong with errors

fn main() {
    println!("Hello, world!");
    let _ = solve(&[Transaction::dispute(ClientId(0), TxId(0))]);
}

fn solve(txs: &[Transaction]) -> ClientAccounts {
    let mut accounts = ClientAccounts::new();
    for t in txs {
        accounts.process(t)
    }
    accounts
}

#[derive(Debug, PartialEq)]
struct ClientAccounts {
    clients: HashMap<ClientId, Account>,
}

impl ClientAccounts {
    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    fn process(&mut self, tx: &Transaction) {
        let client_account = self
            .clients
            .entry(tx.client_id)
            .or_insert(Account::empty(tx.client_id));

        if client_account.is_locked {
            return;
        }

        match tx.tx_type {
            TxType::Withdrawal(amount) => {
                // TODO test this
                // TODO: f32 comparison should have appropriate precision
                if client_account.available < amount {
                    return;
                }
                client_account.available -= amount;
            }
            TxType::Deposit(amount) => {
                client_account.available += amount;
            }
            TxType::Dispute => match client_account.transactions.get_mut(&tx.tx_id) {
                Some(tr) => {
                    // TODO: document this
                    // cannot dispute an already disputed transaction
                    if tr.disputed {
                        return;
                    }

                    let disputed_amount = match tr.tx_type {
                        TxType::Withdrawal(deposited_amount) => deposited_amount,
                        TxType::Deposit(deposited_amount) => deposited_amount,
                        // cannot disput a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    tr.disputed = true;

                    // TODO: f32 comparisons should take into account the required decimal precision
                    if client_account.available < disputed_amount {
                        // do nothing
                        // TODO: log that this happens when user deposits,
                        // then withdraws, then a dispute comes in regarding the initial deposit
                        return;
                    }
                    client_account.available -= disputed_amount;
                    client_account.held += disputed_amount;
                }
                // TODO: This dispute referenced a transaction that did not exist
                // In this situation, do nothing
                // Ideally, Log the output
                None => (),
            },

            TxType::Resolve => match client_account.transactions.get_mut(&tx.tx_id) {
                Some(disputed_tx) => {
                    // TODO: document this
                    // cannot resolve a transaction that is not being disputed
                    if !disputed_tx.disputed {
                        return;
                    }

                    let disputed_amount = match disputed_tx.tx_type {
                        TxType::Withdrawal(amount) | TxType::Deposit(amount) => amount,
                        // cannot resolve a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    disputed_tx.disputed = false;

                    // TODO: f32 comparisons should take into account the required decimal precision
                    if client_account.held < disputed_amount {
                        // TODO: this would be a logic bug. It should be logged and analyzed
                        // Anyways, besides logging, do nothing
                        // This could happen if a dispute is resolved twice
                        return;
                    }
                    client_account.available += disputed_amount;
                    client_account.held -= disputed_amount;
                }
                None => {
                    // Do nothing
                    // resolve transactions that reference an inexistent transaction
                    //   should be discarded
                    return;
                }
            },
            TxType::ChargeBack => {
                match client_account.transactions.get_mut(&tx.tx_id) {
                    Some(disputed_tx) => {
                        // TODO: document this
                        // cannot chargeback a transaction that is not being disputed
                        if !disputed_tx.disputed {
                            return;
                        }

                        let disputed_amount = match disputed_tx.tx_type {
                            TxType::Withdrawal(amount) | TxType::Deposit(amount) => amount,
                            // cannot chargeback a transaction that is neither deposit nor withdrawal
                            _ => return,
                        };

                        // TODO: document this decision
                        // disputed_tx.disputed = true;

                        // TODO: f32 comparisons should take into account the required decimal precision
                        if client_account.held < disputed_amount {
                            // TODO: this would be a logic bug. It should be logged and analyzed
                            // Anyways, besides logging, do nothing
                            // This could happen if a dispute is resolved twice
                            return;
                        }
                        client_account.available += disputed_amount;
                        client_account.held -= disputed_amount;
                    }
                    None => {
                        // Do nothing
                        // resolve transactions that reference an inexistent transaction
                        //   should be discarded
                        return;
                    }
                }
            }
        }
        // Is it better to move the transaction here?
        client_account.transactions.insert(tx.tx_id, tx.clone());
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TxType {
    // TODO: document decimal precision
    Withdrawal(f32),
    Deposit(f32),
    Dispute,
    Resolve,
    ChargeBack,
}

// TODO: explain reasoning, can't math with them, can't mess them up since they're the same type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ClientId(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TxId(u16);

#[derive(Debug, Clone, PartialEq)]
struct Transaction {
    client_id: ClientId,
    tx_id: TxId,
    tx_type: TxType,
    // TODO: document and test disputed transactions
    // TODO: type system allows Dispute/Resolve/Chargback
    // transactions to be disputed, which is not right
    disputed: bool,
}

// TODO: where should I assert that value isn't zero?
impl Transaction {
    fn new(client_id: ClientId, tx_id: TxId, tx_type: TxType) -> Self {
        Self {
            client_id,
            tx_id,
            disputed: false,
            tx_type,
        }
    }

    pub fn deposit(client_id: ClientId, tx_id: TxId, value: f32) -> Self {
        Self::new(client_id, tx_id, TxType::Deposit(value))
    }

    pub fn withdrawal(client_id: ClientId, tx_id: TxId, value: f32) -> Self {
        Self::new(client_id, tx_id, TxType::Withdrawal(value))
    }

    pub fn dispute(client_id: ClientId, tx_id: TxId) -> Self {
        Self::new(client_id, tx_id, TxType::Dispute)
    }

    pub fn resolve(client_id: ClientId, tx_id: TxId) -> Self {
        Self::new(client_id, tx_id, TxType::Resolve)
    }

    pub fn chargeback(client_id: ClientId, tx_id: TxId) -> Self {
        Self::new(client_id, tx_id, TxType::ChargeBack)
    }
}

// TODO: Add check that avaliable + held = total during transaction computation
// If this invariant holds, there is no need to store total
#[derive(PartialEq, Debug)]
struct Account {
    available: f32,
    held: f32,
    is_locked: bool,
    transactions: HashMap<TxId, Transaction>,
}

impl Account {
    fn empty(client_id: ClientId) -> Self {
        Account::unlocked(0.0, 0.0, vec![])
    }

    // TODO: isolate behind trait to be used for testing only
    fn unlocked(available: f32, held: f32, txs: Vec<Transaction>) -> Self {
        Account {
            available,
            held,
            is_locked: false,
            transactions: txs.into_iter().map(|tx| (tx.tx_id, tx)).collect(),
        }
    }

    fn with_available(available: f32, txs: Vec<Transaction>) -> Self {
        Account::unlocked(available, 0.0, txs)
    }
}

/**
 * TODO:
 * - test non existing disputes
 * - test varying precisions for amounts in CSV files
 * - test spacing in outputs
 * - test negative values are not allowed
 */

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    use crate::{solve, Account, ClientAccounts, ClientId, Transaction, TxId};

    fn get_test_cases() -> [(String, TestCase); 12] {
        [
            resolve_only_decs_respective_held(),
            resolve_decs_held_incs_available(),
            dispute_only_affects_selected_client(),
            withdrawal_doesnt_increment_held(),
            deposit_doesnt_increment_held(),
            dispute_holds_funds(),
            no_withdrawal_of_held_funds(),
            no_withdrawal_when_account_is_empty(),
            withdrawal_succeeds_when_same_amount(),
            withdrawal_succeeds_when_less(),
            withdrawal_fails_when_more(),
            deposits_accumulate(),
        ]
    }


    #[test]
    fn run_tests() {
        for t in get_test_cases() {



        }
        // let t = deposits_accumulate();
        // let t = withdrawal_fails_when_more();
        // let t = withdrawal_succeeds_when_less();
        // let t = withdrawal_succeeds_when_less();
        // let t = withdrawal_succeeds_when_same_amount();
        let t = no_withdrawal_of_held_funds();
        let t = no_withdrawal_when_account_is_empty();
        let t = dispute_holds_funds();
        let t = dispute_only_affects_selected_client();
        let result = solve(&t.inputs);
        assert_eq!(result, t.expected_outputs);
    }

    struct TestCase {
        inputs: Vec<Transaction>,
        // TODO: explain HashMap is order agnostic
        expected_outputs: ClientAccounts,
    }

    impl From<HashMap<ClientId, Account>> for ClientAccounts {
        fn from(clients: HashMap<ClientId, Account>) -> Self {
            ClientAccounts { clients }
        }
    }

    // TODO: document this call to cloned
    // Explain why it's needed
    fn tx_of_client(txs: &[Transaction], client_id: ClientId) -> Vec<Transaction> {
        txs.iter()
            .cloned()
            .filter(|tx| tx.client_id == client_id)
            .collect::<Vec<_>>()
    }

    // TODO: test disputes, resolves and chargebacks that reference
    // inexistent transactions, or transactions that are neither a deposit nor a withdrawal

    fn resolve_only_decs_respective_held() -> TestCase {
        let client_id1 = ClientId(0);
        let client_id2 = ClientId(1);
        let disputed_id1 = TxId(3);
        let disputed_id2 = TxId(4);
        let disputed_amount1 = 1.0;
        let disputed_amount2 = 3.0;
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id1, TxId(0), initial_amount),
            Transaction::deposit(client_id2, TxId(1), initial_amount),
            Transaction::deposit(client_id1, disputed_id1, disputed_amount1),
            Transaction::dispute(client_id1, disputed_id1),
            Transaction::deposit(client_id2, disputed_id2, disputed_amount2),
            Transaction::dispute(client_id2, disputed_id2),
            Transaction::resolve(client_id1, disputed_id1),
        ];

        // client 1 dispute has been resolved
        // client 2 dispute has not
        let expected_outputs = HashMap::from_iter([
            (
                client_id1,
                Account::with_available(
                    initial_amount + disputed_amount1,
                    tx_of_client(&inputs, client_id1),
                ),
            ),
            (
                client_id2,
                Account::unlocked(
                    initial_amount,
                    disputed_amount2,
                    tx_of_client(&inputs, client_id2),
                ),
            ),
        ]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn resolve_decs_held_incs_available() -> TestCase {
        let client_id = ClientId(0);
        let disputed_id = TxId(1);
        let disputed_amount = 1.0;
        let withdrawn = 3.0;
        let inputs = vec![
            Transaction::deposit(client_id, disputed_id, disputed_amount),
            Transaction::dispute(client_id, disputed_id),
            Transaction::resolve(client_id, disputed_id),
        ];

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(disputed_amount, tx_of_client(&inputs, client_id)),
        )]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn dispute_only_affects_selected_client() -> TestCase {
        let client_id1 = ClientId(0);
        let client_id2 = ClientId(1);
        let disputed_id1 = TxId(3);
        let disputed_amount = 1.0;
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id1, TxId(0), initial_amount),
            Transaction::deposit(client_id2, TxId(1), initial_amount),
            Transaction::deposit(client_id1, disputed_id1, disputed_amount),
            Transaction::deposit(client_id2, TxId(1), initial_amount),
            Transaction::dispute(client_id1, disputed_id1),
        ];

        // dispute did not affect client 2
        let expected_outputs = HashMap::from_iter([
            (
                client_id1,
                Account::unlocked(
                    initial_amount,
                    disputed_amount,
                    tx_of_client(&inputs, client_id1),
                ),
            ),
            (
                client_id2,
                Account::with_available(initial_amount, tx_of_client(&inputs, client_id2)),
            ),
        ]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn withdrawal_doesnt_increment_held() -> TestCase {
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

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount + withdrawn,
                disputed_amount,
                tx_of_client(&inputs, client_id),
            ),
        )]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn deposit_doesnt_increment_held() -> TestCase {
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

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount + add_amount,
                disputed_amount,
                tx_of_client(&inputs, client_id),
            ),
        )]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn dispute_holds_funds() -> TestCase {
        let client_id = ClientId(0);
        let deposit_id = TxId(1);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), initial_amount),
            Transaction::deposit(client_id, deposit_id, disputed_amount),
            Transaction::dispute(client_id, deposit_id),
        ];

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount,
                disputed_amount,
                tx_of_client(&inputs, client_id),
            ),
        )]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn no_withdrawal_of_held_funds() -> TestCase {
        let client_id = ClientId(0);
        let deposit_id = TxId(0);
        let initial_amount = 10.0;
        let disputed_amount = 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, deposit_id, initial_amount),
            Transaction::deposit(client_id, deposit_id, disputed_amount),
            Transaction::dispute(client_id, deposit_id),
            Transaction::withdrawal(client_id, TxId(1), 1.0),
        ];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount,
                disputed_amount,
                tx_of_client(&inputs, client_id),
            ),
        )]);

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn no_withdrawal_when_account_is_empty() -> TestCase {
        let client_id = ClientId(0);
        let inputs = vec![Transaction::withdrawal(client_id, TxId(1), 1.0)];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(0.0, tx_of_client(&inputs, client_id)),
        )]);
        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn withdrawal_succeeds_when_same_amount() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), val_1),
        ];
        let expected_outputs =
            HashMap::from_iter([(client_id, Account::with_available(0.0, inputs.clone()))]);
        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn withdrawal_succeeds_when_less() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let withdraw = val_1 - 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), withdraw),
        ];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(val_1 - withdraw, tx_of_client(&inputs, client_id)),
        )]);
        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn withdrawal_fails_when_more() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let withdraw = val_1 + 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), withdraw),
        ];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(val_1, vec![inputs[0].clone()]),
        )]);
        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn deposits_accumulate() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let val_2 = val_1 + 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::deposit(client_id, TxId(1), val_2),
            Transaction::deposit(client_id, TxId(2), 0.0),
        ];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(val_1 + val_2, tx_of_client(&inputs, client_id)),
        )]);
        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }
}

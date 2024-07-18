#![feature(hash_set_entry)]
use std::{
    collections::{HashMap, HashSet}, fs::File, hash::Hash, io::{stdout, BufRead, BufReader}, num::ParseFloatError, str::FromStr
};

use csv::Writer;
use serde::Serializer;
use serde_derive::{Deserialize, Serialize};

// TODO: explore cache-friendlier alternatives
//      perhaps a matrix that covered all the transactions that happnned X seconds ago, where X the typicall amount that disputes occurs
// TODO: dispute, resolve, and cashback tx
//       handle tx_ids differently than deposit and withdrawals
//       perhaps auto generate the required IDs
// TODO: sinalize which transactions have gone wrong with errors
// TODO: use faster hashmaps
// TODO: document what would be a better data model for dispute
// resolve and cashback transactions
// TODO: mention pretty_printing bug
// TODO: explain tests need to be underperformant in order to be pure
// TODO: explain decision regarding disputes for withdrawals
// resolves should increment held and decrement available. this is applicable to disputes
// assignment is not clear on what to do this

fn main() {
    let f = File::open("input_file.csv").unwrap();
    let a = BufReader::new(f);
    let mut accounts = ClientAccounts::new();
    for l in a.lines(){
        let tx = l.unwrap().parse().unwrap(); 
        accounts.process(&tx);
    }
    println!("{}", accounts);
}

#[derive(Debug, PartialEq)]
struct ClientAccounts {
    clients: HashMap<ClientId, Account>,
}

impl ClientAccounts {
    fn consume_all(&mut self, txs: &[Transaction]) {
        for t in txs {
            self.process(t)
        }
    }

    fn new() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }

    fn eject_clients(mut self) -> Vec<(ClientId, Account)> {
        let mut a: Vec<_> = self.clients.drain().collect();
        a.sort_by(|(clientid1, _), (clientid2, _)| clientid1.cmp(clientid2));
        a
    }

    // TODO: process is a very large function
    // Document this
    fn process(&mut self, tx: &Transaction) {
        let client_account = self.clients.entry(tx.client).or_insert(Account::empty());

        if client_account.is_locked {
            return;
        }

        match tx.tx_type {
            TxType::Withdrawal(amount) => {
                // TODO test this
                // TODO: f64 comparison should have appropriate precision
                if client_account.available < amount {
                    return;
                }
                client_account.available -= amount;
            }
            TxType::Deposit(amount) => {
                client_account.available += amount;
            }
            TxType::Dispute => match client_account.transactions.get_mut(&tx.tx) {
                Some(tr) => {
                    // TODO: document this
                    // cannot dispute an already disputed transaction
                    if tr.disputed {
                        return;
                    }

                    let disputed_amount = match tr.tx_type {
                        TxType::Withdrawal(_) => {
                            todo!("Undecided regarding disputes to withdrawals")
                        }
                        TxType::Deposit(deposited_amount) => deposited_amount,
                        // cannot disput a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    tr.disputed = true;

                    // TODO: f64 comparisons should take into account the required decimal precision
                    // do nothing
                    // TODO: log that when this does not  happens when user deposits,
                    // then withdraws, then a dispute comes in regarding the initial deposit
                    // Document necessity of rounding here
                    if client_account.available >= disputed_amount {
                        client_account.available -= disputed_amount;
                        client_account.held += disputed_amount;
                    }
                    // do not add transaction to transaction history
                    return;
                }
                // TODO: This dispute referenced a transaction that did not exist
                // In this situation, do nothing
                // Ideally, Log the output
                // do not add transaction to transaction history
                None => return,
            },

            TxType::Resolve => match client_account.transactions.get_mut(&tx.tx) {
                Some(disputed_tx) => {
                    // TODO: document this
                    // cannot resolve a transaction that is not being disputed
                    if !disputed_tx.disputed {
                        return;
                    }

                    let disputed_amount = match disputed_tx.tx_type {
                        TxType::Withdrawal(amount) => amount,
                        TxType::Deposit(_) => todo!("Undecided regarding disputes to withdrawals"),
                        // cannot resolve a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    disputed_tx.disputed = false;

                    // TODO: f64 comparisons should take into account the required decimal precision
                    // TODO: this would be a logic bug. It should be logged and analyzed
                    // Anyways, besides logging, do nothing
                    // This could happen if a dispute is resolved twice
                    if client_account.held >= disputed_amount {
                        client_account.available += disputed_amount;
                        client_account.held -= disputed_amount;
                    }
                    return;
                }
                None => {
                    // Do nothing
                    // resolve transactions that reference an inexistent transaction
                    //   should be discarded
                    return;
                }
            },
            TxType::ChargeBack => {
                match client_account.transactions.get_mut(&tx.tx) {
                    Some(disputed_tx) => {
                        // TODO: document this
                        // cannot chargeback a transaction that is not being disputed
                        if !disputed_tx.disputed {
                            return;
                        }

                        let disputed_amount = match disputed_tx.tx_type {
                            TxType::Withdrawal(_) => {
                                todo!("Undecided regarding disputes to withdrawals")
                            }
                            TxType::Deposit(amount) => amount,
                            // cannot chargeback a transaction that is neither deposit
                            _ => return,
                        };

                        // TODO: document this decision
                        // disputed_tx.disputed = true;

                        // TODO: f64 comparisons should take into account the required decimal precision
                        if client_account.held >= disputed_amount {
                            client_account.held -= disputed_amount;
                            // TODO: this would be a logic bug. It should be logged and analyzed
                            // Anyways, besides logging, do nothing
                            // This could happen if a dispute is resolved twice
                            client_account.is_locked = true;
                        }
                        return;
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
        client_account.transactions.insert(tx.tx, tx.clone());
    }
}

impl std::fmt::Display for ClientAccounts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client, available, held, total, locked\n")?;
        for (id, account) in &self.clients {
            // using debug implementatoin of ClientId, might be antipattern
            write!(f, "{:?}, {}\n", id.0, account)?;
        }
        std::fmt::Result::Ok(())
    }
}

#[derive(strum_macros::Display, Deserialize, Debug, Clone, PartialEq)]
enum TxType {
    // TODO: document decimal precision
    // f64 would lose precision at XXXX.YYY
    // f64 allows much larger numbers XXXXXXXXXXX.YYYY
    Withdrawal(f64),
    Deposit(f64),
    Dispute,
    Resolve,
    ChargeBack,
}

// TODO: explain reasoning, can't math with them, can't mess them up since they're the same type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ClientId(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TxId(u16);

#[derive(Debug, Clone, PartialEq)]
struct Transaction {
    tx_type: TxType,
    client: ClientId,
    tx: TxId,
    // TODO: document and test disputed transactions
    // TODO: type system allows Dispute/Resolve/Chargback
    // transactions to be disputed, which is not right
    disputed: bool,
}

// TODO: this functions performance can be improved
impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let amount = match self.tx_type {
            TxType::Withdrawal(amount) | TxType::Deposit(amount) => amount.to_string(),
            _ => " ".to_owned(),
        };
        write!(
            f,
            "{}, {:.4}, {:.4}, {} ",
            self.tx_type, self.client.0, self.tx.0, amount
        )
    }
}

// TODO: ask me how this can be improved
impl FromStr for Transaction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut fields: Vec<_> = s.split(',').collect();
        // TODO: handle panics here
        if fields.len() == 0 {
            return Err("Cannot parse empty transaction string".to_owned());
        }

        let client = ClientId(
            fields
                .get(1)
                .ok_or("Incorrect transaction formatting")?
                .trim()
                .parse::<u16>()
                .map_err(|e| e.to_string())?,
        );
        let tx = TxId(
            fields
                .get(2)
                .ok_or("Incorrect transaction formatting")?
                .trim()
                .parse::<u16>()
                .map_err(|e| e.to_string())?,
        );

        let tx_type = if fields.len() == 4 {
            match fields.get(0) {
                Some(&"deposit") => {
                    TxType::Deposit(fields[3].trim().parse::<f64>().map_err(|e| e.to_string())?)
                }
                Some(&"withdrawal") => {
                    TxType::Withdrawal(fields[3].trim().parse::<f64>().map_err(|e| e.to_string())?)
                }
                Some(_) => {
                    return Err(
                        "disputes, resolves and chargebacks cannot contain amounts".to_owned()
                    )
                }
                None => unreachable!("Asserted above that len == 4"),
            }
        } else if fields.len() == 3 {
            match fields.get(0) {
                Some(&"dispute") => TxType::Dispute,
                Some(&"chargeback") => TxType::ChargeBack,
                Some(&"resolve") => TxType::Resolve,
                Some(_) => return Err("deposits and withdrawals must contain amounts".to_owned()),
                None => unreachable!("Asserted above that len == 3"),
            }
        } else {
            return Err("Transaction strings must have either 3 or 4 fields".to_owned());
        };
        Ok(Transaction {
            client,
            tx,
            tx_type,
            // all transactions start out as not disputed
            disputed: false,
        })
    }
}

// TODO: where should I assert that value isn't zero?
impl Transaction {
    fn new(client_id: ClientId, tx_id: TxId, tx_type: TxType) -> Self {
        Self {
            client: client_id,
            tx: tx_id,
            disputed: false,
            tx_type,
        }
    }

    pub fn deposit(client_id: ClientId, tx_id: TxId, value: f64) -> Self {
        Self::new(client_id, tx_id, TxType::Deposit(value))
    }

    pub fn withdrawal(client_id: ClientId, tx_id: TxId, value: f64) -> Self {
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
    available: f64,
    held: f64,
    is_locked: bool,
    transactions: HashMap<TxId, Transaction>,
}


impl Account {
    fn empty() -> Self {
        Account::unlocked(0.0, 0.0, vec![])
    }

    // TODO: isolate behind trait to be used for testing only
    fn unlocked(available: f64, held: f64, txs: Vec<Transaction>) -> Self {
        Account::new(available, held, txs, false)
    }

    fn new(available: f64, held: f64, txs: Vec<Transaction>, is_locked: bool) -> Self {
        Account {
            available,
            held,
            is_locked,
            transactions: txs.into_iter().map(|tx| (tx.tx, tx)).collect(),
        }
    }

    fn with_available(available: f64, txs: Vec<Transaction>) -> Self {
        Account::unlocked(available, 0.0, txs)
    }
}

impl std::fmt::Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}, {:.4}, {:.4}, {}", self.available, self.held, self.available + self.held, self.is_locked)
    }
}

/**
 * TODO:
 * - test non existing disputes
 * - test varying precisions for amounts in CSV files
 * - test spacing in outputs
 * - test negative values are not allowed
 * - test dispute, resolve and chargeback can only reference
 *      transactions specific to the client they reference
 */

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;

    use crate::{Account, ClientAccounts, ClientId, Transaction, TxId, TxType};

    struct TestCase {
        inputs: Vec<Transaction>,
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
            .filter(|tx| {
                tx.client == client_id
                    && matches!(tx.tx_type, TxType::Deposit(_) | TxType::Withdrawal(_))
            })
            .collect::<Vec<_>>()
    }

    // TODO: test disputes, resolves and chargebacks that reference
    // inexistent transactions, or transactions that are neither a deposit nor a withdrawal

    #[test]
    fn disputes_are_non_idempotent_after_resolve() {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let tx_id = TxId(0);
        let inputs = vec![
            Transaction::deposit(client_id0, tx_id, initial_amount),
            Transaction::dispute(client_id0, tx_id),
        ];

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].disputed = true;

        let expected_outputs = ClientAccounts::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions.clone()),
        )]));
        assert_eq!(actual, expected_outputs);

        actual.process(&Transaction::resolve(client_id0, tx_id));

        client_transactions[0].disputed = false;
        let expected_outputs = ClientAccounts::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(10.0, 0.0, client_transactions.clone()),
        )]));
        assert_eq!(actual, expected_outputs);

        actual.process(&Transaction::dispute(client_id0, tx_id));

        client_transactions[0].disputed = true;
        let expected_outputs = ClientAccounts::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions),
        )]));
        assert_eq!(actual, expected_outputs);
    }

    #[test]
    fn chargeback_withdraws_and_locks_account() {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
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
        client_transactions[0].disputed = true;
        // remove last deposit, has it shouldn't be present in transaction history
        client_transactions.pop();

        let expected_outputs = HashMap::from_iter([(
            client_id0,
            Account::new(0.0, 0.0, client_transactions, true),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, ClientAccounts::from(expected_outputs));
    }

    #[test]
    fn disputes_are_idempotent_before_resolve() {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let tx_id = TxId(0);
        let inputs = vec![
            Transaction::deposit(client_id0, tx_id, initial_amount),
            Transaction::dispute(client_id0, tx_id),
            Transaction::dispute(client_id0, tx_id),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, ClientAccounts::from(expected_outputs));
    }

    #[test]
    fn unexistent_resolve_reference() {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::dispute(client_id0, TxId(0)),
            Transaction::resolve(client_id0, TxId(100)),
        ];

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, ClientAccounts::from(expected_outputs));
    }

    #[test]
    fn unexistent_dispute_reference() {
        let client_id0 = ClientId(0);
        let initial_amount = 10.0;
        let inputs = vec![
            Transaction::deposit(client_id0, TxId(0), initial_amount),
            Transaction::dispute(client_id0, TxId(100)),
        ];

        let expected_outputs = HashMap::from_iter([(
            client_id0,
            Account::with_available(initial_amount, tx_of_client(&inputs, client_id0)),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, ClientAccounts::from(expected_outputs));
    }

    #[test]
    fn resolve_only_decs_respective_held() {
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
        client1_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([
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

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, ClientAccounts::from(expected_outputs));
    }

    #[test]
    fn resolve_decs_held_incs_available() {
        let client_id = ClientId(0);
        let disputed_id = TxId(1);
        let disputed_amount = 1.0;
        let inputs = vec![
            Transaction::deposit(client_id, disputed_id, disputed_amount),
            Transaction::dispute(client_id, disputed_id),
            Transaction::resolve(client_id, disputed_id),
        ];

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::with_available(disputed_amount, tx_of_client(&inputs, client_id)),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn dispute_only_affects_selected_client() {
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
        client0_transactions[1].disputed = true;

        // dispute did not affect client 2
        let expected_outputs = HashMap::from_iter([
            (
                client_id0,
                Account::unlocked(initial_amount, disputed_amount, client0_transactions),
            ),
            (
                client_id1,
                Account::with_available(initial_amount * 2.0, tx_of_client(&inputs, client_id1)),
            ),
        ]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(
            actual.eject_clients(),
            ClientAccounts::from(expected_outputs).eject_clients()
        );
    }

    #[test]
    fn withdrawal_doesnt_decrement_held() {
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
        client_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount - withdrawn,
                disputed_amount,
                client_transactions,
            ),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn deposit_doesnt_increment_held() {
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
        client_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount + add_amount,
                disputed_amount,
                client_transactions,
            ),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn dispute_holds_funds() {
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
        client_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(initial_amount, disputed_amount, client_transactions),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn no_withdrawal_of_held_funds() {
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
        client_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(
                initial_amount - withdrawn_amount,
                disputed_amount,
                client_transactions,
            ),
        )]);

        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn withdrawal_fails_when_account_is_empty() {
        let client_id = ClientId(0);
        let inputs = vec![Transaction::withdrawal(client_id, TxId(1), 1.0)];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            // not added to history
            Account::with_available(0.0, vec![]),
        )]);
        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn withdrawal_succeeds_when_same_as_available() {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), val_1),
        ];
        let expected_outputs =
            HashMap::from_iter([(client_id, Account::with_available(0.0, inputs.clone()))]);
        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn withdrawal_succeeds_when_less_than_available() {
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
        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn withdrawal_fails_when_more() {
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
        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }

    #[test]
    fn deposits_accumulate() {
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
        let mut actual = ClientAccounts::new();
        actual.consume_all(&inputs);
        assert_eq!(actual, expected_outputs.into());
    }
}

#![feature(hash_set_entry)]
#![feature(iter_intersperse)]
#![feature(concat_idents)]
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    hash::Hash,
    io::{stdout, BufRead, BufReader, Write},
    num::{ParseFloatError, ParseIntError},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use csv::Writer;
use futures::{stream, StreamExt};
use serde::Serializer;
use serde_derive::{Deserialize, Serialize};
use tokio::{
    io::DuplexStream,
    sync::{Mutex, RwLock}, task::JoinHandle,
};

type TokioSender<T> = tokio::sync::mpsc::Sender<T>;


// TODO: explore cache-friendlier alternatives
//      perhaps a matrix that covered all the transactions that happnned X seconds ago, where X the typicall amount that disputes occurs
// TODO: Explain that dispute, resolve, and cashback are not stored, because they have no id
// TODO: sinalize which transactions have gone wrong with errors
// TODO: use faster hashmaps
// TODO: document what would be a better data model for dispute
// resolve and cashback transactions
// TODO: mention pretty_printing bug
// TODO: explain tests need to be underperformant in order to be pure
// TODO: explain decision regarding disputes for withdrawals
// resolves should increment held and decrement available. this is applicable to disputes
// assignment is not clear on what to do this
// TODO: explain lack of Serde

#[tokio::main]
async fn main() {
    let file_name = std::env::args().nth(1).unwrap();
    let f = File::open(file_name).unwrap();
    let reader = BufReader::new(f);
    let lines = reader.lines().skip(1);

    let mut accounts = HashMap::<
        ClientId,
        (
            TokioSender<String>,
            JoinHandle<Account>,
        ),
    >::new();

    for l in lines {
        let l = l.unwrap();
        let id = get_id(&l);
        if let Some((sender, _)) = accounts.get(&id) {
            match sender.send(l).await {
                Ok(_) => (),
                Err(_) => todo!("What errors cna be thrown?"),
            }
        } else {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
            accounts.insert(
                id,
                (
                    sender.clone(),
                    tokio::spawn(async move {
                        let mut account = Account::empty();
                        loop {
                            match receiver.recv().await {
                                Some(tx_str) => {
                                    let tx = tx_str.parse().unwrap();
                                    tokio::time::sleep(Duration::from_secs(10)).await;
                                    account.process(tx);
                                }
                                None => break,
                            }
                        }
                        account
                    }),
                ),
            );
            match sender.send(l).await {
                Ok(_) => (),
                Err(_) => todo!("What errors cna be thrown?"),
            }
        }

        // handles.push(tokio::spawn(async move {
        //     let tx = l.parse().unwrap();
        //     let mut h = handler_arc.lock().await;
        //     h.process(&tx);
        //     // drop(h);
        //     // tokio::time::sleep(Duration::from_secs(10)).await;
        //     // std::thread::sleep(Duration::from_secs(10));
        // }));
    }

    let mut result = vec![];
    for (i, (s, handle)) in accounts {
        // let _r = s.send(Message::CloseChannel).await ;
        drop(s);
        result.push((i, handle.await.unwrap()));
    }


    let _ = stdout().write(print_results(result).as_bytes()).unwrap();
}



fn print_results(accounts: Vec<(ClientId, Account)>) -> String {
    let mut result = format!("client, available, held, total, locked\n");
    for (id, account) in accounts{
        // using debug implementatoin of ClientId, might be antipattern
        result += &format!( "{:?}, {}\n", id.0, account);
    }
    result
}

fn get_id(line: &str) -> ClientId {
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

    async fn process(&mut self, tx_str: &str) {
        let id = get_id(tx_str);
        self.clients.entry(&id)
        if let Some((sender, _)) = self.clients.get(&id) {
            match sender.send(l).await {
                Ok(_) => (),
                Err(_) => todo!("What errors cna be thrown?"),
            }
        } else {
            let (sender, mut receiver) = tokio::sync::mpsc::channel(100);
            accounts.insert(
                id,
                (
                    sender.clone(),
                    tokio::spawn(async move {
                        let mut account = Account::empty();
                        loop {
                            match receiver.recv().await {
                                Some(tx_str) => {
                                    let tx = tx_str.parse().unwrap();
                                    tokio::time::sleep(Duration::from_secs(10)).await;
                                    account.process(tx);
                                }
                                None => break,
                            }
                        }
                        account
                    }),
                ),
            );
            match sender.send(l).await {
                Ok(_) => (),
                Err(_) => todo!("What errors cna be thrown?"),
            }
        }
    }

    fn eject_clients(mut self) -> Vec<(ClientId, Account)> {
        let mut a: Vec<_> = self.clients.drain().collect();
        a.sort_by(|(clientid1, _), (clientid2, _)| clientid1.cmp(clientid2));
        a
    }

    fn cmp_without_history(&self, other: &Self) -> bool {
        for (client, account) in self.clients.iter() {
            if let Some(other_client) = other.clients.get(client) {
                if !account.are_same_without_transactions(other_client) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}

impl FromStr for AccountHandler {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut clients = HashMap::new();
        for l in s.lines().skip(1) {
            let mut iter = l.split(',');
            let client_id = ClientId(iter.next().unwrap().parse().unwrap());
            let available = iter.next().unwrap().trim().parse().unwrap();
            let held = iter.next().unwrap().trim().parse().unwrap();
            let _skip_total = iter.next();
            let is_locked = iter.next().unwrap().trim().parse().unwrap();
            clients.insert(client_id, Account::new(available, held, vec![], is_locked));
        }
        Ok(AccountHandler { clients })
    }
}

impl std::fmt::Display for AccountHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client, available, held, total, locked\n")?;
        for (id, account) in &self.clients {
            // using debug implementatoin of ClientId, might be antipattern
            write!(f, "{:?}, {}\n", id.0, account)?;
        }
        std::fmt::Result::Ok(())
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
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

impl std::fmt::Display for TxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            TxType::Withdrawal(_) => "withdrawal",
            TxType::Deposit(_) => "deposit",
            TxType::Dispute => "dispute",
            TxType::Resolve => "resolve",
            TxType::ChargeBack => "chargback",
        };
        write!(f, "{string}")
    }
}

// TODO: explain reasoning, can't math with them, can't mess them up since they're the same type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ClientId(u16);

impl FromStr for ClientId {
    // TODO better here
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim().parse()?))
    }
}

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

const PRECISION_FACTOR: f64 = 1e4;

fn round_equals(first: f64, second: f64) -> bool {
    (first * PRECISION_FACTOR).round() / PRECISION_FACTOR
        == (second * PRECISION_FACTOR).round() / PRECISION_FACTOR
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
    fn process(&mut self, tx: Transaction) {
        if self.is_locked {
            return;
        }

        match tx.tx_type {
            TxType::Withdrawal(amount) => {
                // TODO test this
                // TODO: f64 comparison should have appropriate precision
                if self.available < amount {
                    return;
                }
                self.available -= amount;
            }
            TxType::Deposit(amount) => {
                self.available += amount;
            }
            TxType::Dispute => match self.transactions.get_mut(&tx.tx) {
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
                    if self.available >= disputed_amount {
                        self.available -= disputed_amount;
                        self.held += disputed_amount;
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

            TxType::Resolve => match self.transactions.get_mut(&tx.tx) {
                Some(disputed_tx) => {
                    // TODO: document this
                    // cannot resolve a transaction that is not being disputed
                    if !disputed_tx.disputed {
                        return;
                    }

                    let disputed_amount = match disputed_tx.tx_type {
                        TxType::Withdrawal(_) => {
                            todo!("Undecided regarding disputes to withdrawals")
                        }
                        TxType::Deposit(amount) => amount,
                        // cannot resolve a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    disputed_tx.disputed = false;

                    // TODO: f64 comparisons should take into account the required decimal precision
                    // TODO: this would be a logic bug. It should be logged and analyzed
                    // Anyways, besides logging, do nothing
                    // This could happen if a dispute is resolved twice
                    if self.held >= disputed_amount {
                        self.available += disputed_amount;
                        self.held -= disputed_amount;
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
                match self.transactions.get_mut(&tx.tx) {
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
                        if self.held >= disputed_amount {
                            self.held -= disputed_amount;
                            // TODO: this would be a logic bug. It should be logged and analyzed
                            // Anyways, besides logging, do nothing
                            // This could happen if a dispute is resolved twice
                            self.is_locked = true;
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
        self.transactions.insert(tx.tx, tx.clone());
    }

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

    fn are_same_without_transactions(&self, other: &Self) -> bool {
        round_equals(other.available, self.available)
            && round_equals(other.held, self.held)
            && other.is_locked == self.is_locked
    }
}

impl std::fmt::Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.4}, {:.4}, {:.4}, {}",
            self.available,
            self.held,
            self.available + self.held,
            self.is_locked
        )
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
    use std::{
        collections::HashMap,
        io::{stderr, stdin, Read, Write},
        process::Command,
    };
    use tokio::runtime::Handle;

    use crate::{Account, AccountHandler, ClientId, Transaction, TxId, TxType};

    struct TestCase {
        inputs: Vec<Transaction>,
        expected_outputs: AccountHandler,
    }

    impl From<HashMap<ClientId, Account>> for AccountHandler {
        fn from(clients: HashMap<ClientId, Account>) -> Self {
            AccountHandler { clients }
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

        let mut actual = AccountHandler::new();
        actual.consume_all(&inputs);

        let mut client_transactions = tx_of_client(&inputs, client_id0);
        client_transactions[0].disputed = true;

        let expected_outputs = AccountHandler::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions.clone()),
        )]));
        assert_eq!(actual, expected_outputs);

        actual.process(&Transaction::resolve(client_id0, tx_id));

        client_transactions[0].disputed = false;
        let expected_outputs = AccountHandler::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(10.0, 0.0, client_transactions.clone()),
        )]));
        assert_eq!(actual, expected_outputs);

        actual.process(&Transaction::dispute(client_id0, tx_id));

        client_transactions[0].disputed = true;
        let expected_outputs = AccountHandler::from(HashMap::from_iter([(
            client_id0,
            Account::unlocked(0.0, 10.0, client_transactions),
        )]));
        assert_eq!(actual, expected_outputs);
    }

    macro_rules! test_cases {
        () => {
            chargeback_withdraws_and_locks_account,
            disputes_are_idempotent_before_resolve,
            disputes_are_non_idempotent_after_resolve,
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
            deposits_accumulate
        };
    }

    // TODO: this could be paralelled with macros
    #[test]
    fn run_integration_tests() {
        let test_cases = [
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

        for i in 0..10 {

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
            let actual: AccountHandler = result.parse().unwrap();
            assert!(actual.cmp_without_history(&expected_outputs));
        }

        stderr().write(format!("finnished test case run number{i}").as_bytes());
        }

    }

    fn chargeback_withdraws_and_locks_account() -> TestCase {
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


        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn disputes_are_idempotent_before_resolve() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn unexistent_resolve_reference() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn unexistent_dispute_reference() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn resolve_only_decs_respective_held() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn resolve_decs_held_incs_available() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn dispute_only_affects_selected_client() -> TestCase {
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

        TestCase {
            inputs,
            expected_outputs: expected_outputs.into(),
        }
    }

    fn withdrawal_doesnt_decrement_held() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
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

        let mut client_transactions = tx_of_client(&inputs, client_id);
        client_transactions[1].disputed = true;

        let expected_outputs = HashMap::from_iter([(
            client_id,
            Account::unlocked(initial_amount, disputed_amount, client_transactions),
        )]);

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn no_withdrawal_of_held_funds() -> TestCase {
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

        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn withdrawal_fails_when_account_is_empty() -> TestCase {
        let client_id = ClientId(0);
        let inputs = vec![Transaction::withdrawal(client_id, TxId(1), 1.0)];
        let expected_outputs = HashMap::from_iter([(
            client_id,
            // not added to history
            Account::with_available(0.0, vec![]),
        )]);
        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn withdrawal_succeeds_when_same_as_available() -> TestCase {
        let client_id = ClientId(0);
        let val_1 = 1.2;
        let inputs = vec![
            Transaction::deposit(client_id, TxId(0), val_1),
            Transaction::withdrawal(client_id, TxId(1), val_1),
        ];
        let expected_outputs =
            HashMap::from_iter([(client_id, Account::with_available(0.0, inputs.clone()))]);
        TestCase {
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }

    fn withdrawal_succeeds_when_less_than_available() -> TestCase {
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
            expected_outputs: expected_outputs.into(),
            inputs,
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
            expected_outputs: expected_outputs.into(),
            inputs,
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
            expected_outputs: expected_outputs.into(),
            inputs,
        }
    }
}

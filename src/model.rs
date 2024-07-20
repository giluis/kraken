use std::{collections::HashMap, num::ParseIntError, str::FromStr};

use crate::round_equals;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxId(pub u16);

// TODO: explain reasoning, can't math with them, can't mess them up since they're the same type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub enum TxType {
    // TODO: document decimal precision
    // f64 would lose precision at XXXX.YYY
    // f64 allows much larger numbers XXXXXXXXXXX.YYYY
    Withdrawal(f64),
    Deposit(f64),
    Dispute,
    Resolve,
    ChargeBack,
}

impl TxType {
}

impl std::fmt::Display for TxType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let string = match self {
            TxType::Withdrawal(_) => "withdrawal",
            TxType::Deposit(_) => "deposit",
            TxType::Dispute => "dispute",
            TxType::Resolve => "resolve",
            TxType::ChargeBack => "chargeback",
        };
        write!(f, "{string}")
    }
}

impl FromStr for ClientId {
    type Err = ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.trim().parse()?))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
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
        let mut result = format!("{}, {:.4}, {:.4}", self.tx_type, self.client.0, self.tx.0);
        if let TxType::Withdrawal(amount) | TxType::Deposit(amount) = self.tx_type {
            result += ",";
            result += &amount.to_string();
        }

        write!(f, "{result}")
    }
}

// TODO: ask me how this can be improved
impl FromStr for Transaction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = s.split(',').collect();
        // TODO: handle panics here
        if fields.is_empty(){
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
            match fields.first() {
                Some(&"deposit") => {
                    TxType::Deposit(fields[3].trim().parse::<f64>().map_err(|e| e.to_string())?)
                }
                Some(&"withdrawal") => {
                    TxType::Withdrawal(fields[3].trim().parse::<f64>().map_err(|e| e.to_string())?)
                }
                Some(_) => {
                    dbg!(&s);
                    return Err(
                        "disputes, resolves and chargebacks cannot contain amounts".to_owned()
                    );
                }
                None => unreachable!("Asserted above that len == 4"),
            }
        } else if fields.len() == 3 {
            match fields.first() {
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
    pub fn new(client_id: ClientId, tx_id: TxId, tx_type: TxType) -> Self {
        Self {
            client: client_id,
            tx: tx_id,
            disputed: false,
            tx_type,
        }
    }

    pub fn set_disputed(&mut self, disputed: bool) {
        self.disputed = disputed;
    }

    pub fn client(&self) -> ClientId {
        self.client
    }

    pub fn is_deposit_or_withdrawal(&self) -> bool {
        matches!(self.tx_type, TxType::Deposit(_) | TxType::Withdrawal(_))
    }
}

#[derive(PartialEq, Debug)]
pub struct Account {
    available: f64,
    held: f64,
    is_locked: bool,
    transactions: HashMap<TxId, Transaction>,
}

impl Account {
    // TODO: make this functoin clearer and smaller
    pub fn process(&mut self, tx: Transaction) {
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
        self.transactions.insert(tx.tx, tx.clone());
    }

    pub fn empty() -> Self {
        Account::unlocked(0.0, 0.0, vec![])
    }

    // TODO: isolate behind trait to be used for testing only
    pub fn unlocked(available: f64, held: f64, txs: Vec<Transaction>) -> Self {
        Account::new(available, held, txs, false)
    }

    pub fn new(available: f64, held: f64, txs: Vec<Transaction>, is_locked: bool) -> Self {
        Account {
            available,
            held,
            is_locked,
            transactions: txs.into_iter().map(|tx| (tx.tx, tx)).collect(),
        }
    }

    pub fn with_available(available: f64, txs: Vec<Transaction>) -> Self {
        Account::unlocked(available, 0.0, txs)
    }

    pub fn are_same_without_transactions(&self, other: &Self) -> bool {
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

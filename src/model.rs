use std::{collections::HashMap, num::ParseIntError, str::FromStr};

use crate::{round_equals, PRECISION_FACTOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(pub u16);

#[derive(Debug, Clone, PartialEq)]
pub enum TxType {
    Withdrawal(f64),
    Deposit(f64),
    Dispute,
    Resolve,
    ChargeBack,
}

impl TxType {}

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

// ask me how this can be improved
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    tx_type: TxType,
    client: ClientId,
    tx: TxId,
    is_disputed: bool,
}

// ask me how this can be improved
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

// ask me how this can be improved
impl FromStr for Transaction {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = s.split(',').collect();
        if fields.is_empty() {
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
                .parse::<u32>()
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
            is_disputed: false,
        })
    }
}

impl Transaction {
    pub fn new(client_id: ClientId, tx_id: TxId, tx_type: TxType) -> Self {
        Self {
            client: client_id,
            tx: tx_id,
            is_disputed: false,
            tx_type,
        }
    }

    pub fn set_disputed(&mut self, disputed: bool) {
        self.is_disputed = disputed;
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
    /**
     * Rounds available and held to 4 decimal places.
     */
    pub fn round(&mut self) {
        self.available = (self.available * PRECISION_FACTOR).round() / PRECISION_FACTOR;
        self.held = (self.held * PRECISION_FACTOR).round() / PRECISION_FACTOR;
    }

    // ask me how this can be improved
    pub fn process(&mut self, tx: Transaction) {
        if self.is_locked {
            return;
        }

        match tx.tx_type {
            TxType::Withdrawal(amount) => {
                if self.available < amount {
                    return;
                }
                self.available -= amount;
                self.transactions.insert(tx.tx, tx.clone());
            }
            TxType::Deposit(amount) => {
                self.available += amount;
                self.transactions.insert(tx.tx, tx.clone());
            }
            TxType::Dispute => match self.transactions.get_mut(&tx.tx) {
                Some(referenced_tx) => {
                    // Cannot dispute an already disputed transaction
                    // Do nothing
                    if referenced_tx.is_disputed {
                        return;
                    }

                    let disputed_amount = match referenced_tx.tx_type {
                        TxType::Withdrawal(_) => {
                            // Though the assignment is not explicitly against it,
                            // i decided that withdrawal transactions cannot be disputed
                            // So, do nothing
                            return;
                        }
                        TxType::Deposit(deposited_amount) => deposited_amount,
                        // Cannot disput a transaction that is neither deposit nor withdrawal
                        _ => return,
                    };

                    referenced_tx.is_disputed = true;

                    if self.available >= disputed_amount {
                        self.available -= disputed_amount;
                        self.held += disputed_amount;
                    } else {
                        // This could happen if a dispute is referencing funds that have already been withdrawn
                        // Assignment isn't clear here on what to do. I opt to do nothing
                        return;
                    }
                }
                // This dispute referenced a transaction that did not exist
                // Do nothing
                None => return,
            },

            TxType::Resolve => match self.transactions.get_mut(&tx.tx) {
                Some(disputed_tx) => {
                    // Cannot resolve a transaction that is not being disputed
                    // Do nothing
                    if !disputed_tx.is_disputed {
                        return;
                    }

                    let disputed_amount = match disputed_tx.tx_type {
                        TxType::Withdrawal(_) => {
                            todo!("Undecided regarding disputes to withdrawals")
                        }
                        TxType::Deposit(amount) => amount,
                        // Cannot resolve a transaction that is neither deposit nor withdrawal
                        // Do nothing
                        _ => return,
                    };

                    disputed_tx.is_disputed = false;

                    if self.held >= disputed_amount {
                        self.available += disputed_amount;
                        self.held -= disputed_amount;
                    } else {
                        // This would be a logic bug.
                        // It could happen if a dispute was resolved twice
                        // That shouldn't happen
                        // I opt to do nothing in this case.
                        // Ideally, this would be logged for analysis
                        return;
                    }
                }
                //  Cannot resolve an innexistent transaction
                //  Do nothing
                None => return,
            },
            TxType::ChargeBack => {
                match self.transactions.get_mut(&tx.tx) {
                    Some(disputed_tx) => {
                        // Cannot chargeback a transaction that is not being disputed
                        // Do nothing
                        if !disputed_tx.is_disputed {
                            return;
                        }

                        let disputed_amount = match disputed_tx.tx_type {
                            TxType::Withdrawal(_) => {
                                todo!("Undecided regarding disputes to withdrawals")
                            }
                            TxType::Deposit(amount) => amount,
                            // Cannot chargeback a transaction that is neither deposit
                            // Do nothing
                            _ => return,
                        };

                        // Works either way, since account will be locked anyway
                        // disputed_tx.disputed = true;

                        if self.held >= disputed_amount {
                            self.held -= disputed_amount;
                        } else {
                            // This would be a logic bug.
                            // It could if a dispute was resolved and chargedback
                            // The assignemnt does not mention what to do in this situation
                            return;
                        }
                        // Locking should happen either way
                        // either because of chargeback
                        // or bug mentioned above
                        self.is_locked = true;
                    }
                    // Chargebacks that cannot reference an inexistent transaction
                    // Do nothing
                    None => return,
                }
            }
        }
        // Whenever a transaction is correctly processed, 
        // its values should be rounded to the appropriate precision.
        // This prevents precision errors from accumulating
        self.round()
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

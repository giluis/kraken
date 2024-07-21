# Luís Gil's solution to \<_redacted_\>'s Rust Test

## How to run

```rust
# prints client account balances to stdout
cargo run -- <filename>
```
If any dependency problem arises, just contact me and I'll provide a Dockerfile.

## Concurrency Strategy

I took some time to find a concurrency strategy that actually produced correct results.  
The issue is that transactions for each client must be processed (and finnish processing) in the order in which they are received.  
So, I spawn a task to handle transactions respective to each client, queueing each incoming task through a channel.


## Testing

This particular assignment has many edge cases:
- What if disputes are resolved twice?
- What if money is withdrawn before a dispute comes, referencing the transaction that deposited it?

I tried to test as thoroughly as possible. 
A few things were left untested: 

- varying precisions for amounts (at 4 digits precision, no issues should arise with f64)
- spacing in outputs (solution should work anyways)
- negative values are not allowed (I assumed none would appear)

Though I have good reason to believe these work

## Questions you should ask me

Why did I ...

1. ... not use Serde?
2. ... used the TestCase pattern in tests?
3. ... use ClientId / TxId rather than u16?
4. ... chose to not store disputes, resolves and cb's?
5. ... opted for f64 for amounts?
6. ... decide withdrawals cannot be disputed?

You can find a few more questions in the code, as comments!


## If I had more time, I would...

1. ... create an Amount struct that always keeps required precision 
        (if such a struct doesn't already exist as a crate)
2. ... have a macro that generates infividual integration tests, rather than a single function that runs them all 
3. ... point 2, but for unit tests as well. 
4. ... have randomized task scheduling tests, rather than running integration tests N times
5. ... explore a cache-friendly / data oriented option, rather than `HashMap<ClientId, (Sender, Account)>`
    (each account has an array of transactions)
6. ... improve the error handling for transactions processing. Current implementation is poor and inconsistent
7. ... explore faster `HashMap` alternatives, like [ahash](https://crates.io/crates/ahash)
8. ... add larger tests with dozens of transactions and clients
9. ... use Serde


## About the edit on Saturday

On Saturday (one day after deadline), after I had submitted a functioning solution, I made the following edits, which you can check in git history:

- addded, removed and updated comments for clarity 
- refactored `Account::process` for clarity
- added explicit rounding to 4 digits of precision
    - my test cases did not take floating-point error accumulation. This prevents that


## Conclusion
Fun challenge. Perhaps a bit too overengineered on my part.  
Let me know your thoughts.

Thanks for the opportunity 🙂
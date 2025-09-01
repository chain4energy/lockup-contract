# ChargEra Lockup Contract

[![License: Apache-2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

A CosmWasm smart contract for managing time-locked token deposits with tiered rewards and progressive APR increases.

## Overview

The Lockup Contract is a CosmWasm smart contract designed to securely manage time-locked token deposits. Users lock tokens for a fixed duration to earn rewards based on a tiered system. The APR increases over time to incentivize long-term holding. The contract includes robust security features like fund sufficiency validation and multi-admin support.

For a detailed breakdown of features and functionality, please see the documentation.

## Documentation

Detailed documentation is available in the `docs/` directory:

- **[Code Overview](./docs/code_overview.md)**: An in-depth explanation of the contract's architecture, data structures, and core functions.
- **[Test Suite Overview](./docs/test_suite_overview.md)**: A guide to the testing strategy and key test cases.
- **[Functional Requirements](./docs/functional_requirements.md)**: A list of the contract's functional specifications.
- **[CLI Usage Guide](./docs/cli_usage.md)**: Instructions on how to deploy and interact with the contract using the `c4ed` CLI.

## Contract Structure

- `src/contract.rs`: Main contract entry points (`instantiate`, `execute`, `query`).
- `src/error.rs`: Custom error types for the contract.
- `src/state.rs`: State management, defining the core data structures like `Config`, `TierConfig`, and `Lockup`.
- `src/msg.rs`: Defines the `InstantiateMsg`, `ExecuteMsg`, and `QueryMsg` types for interacting with the contract.
- `src/helpers.rs`: Helper functions used across the contract.
- `src/tests/`: Contains unit and integration tests.

## Message Types

### InstantiateMsg
```rust
pub struct InstantiateMsg {
    pub admins: Option<Vec<String>>,
    pub denom: String,
    pub lockup_duration_seconds: u64,
    pub tier_config: TierConfig,
}
```

### ExecuteMsg
```rust
pub enum ExecuteMsg {
    Lock {},
    UnlockPrincipal {},
    ClaimRewards {},
    DepositRewards {},
}
```

### QueryMsg
```rust
pub enum QueryMsg {
    GetConfig {},
    GetLockup { address: String },
    GetClaimableRewards { address: String },
    GetAllLockups {},
    GetDepositedRewards {},
    GetSumLockupsAndDeposits {},
    GetAllRewards {},
    CheckCoinAvailability {},
}
```

## Building and Testing

### Build
To build the contract, run the following command:
```bash
cargo wasm
```
This will produce an optimized WASM binary in the `artifacts/` directory.

### Test
To run the test suite:
```bash
cargo test
```

## Deployment Guide

For detailed instructions on how to deploy and interact with the contract on a live network, please refer to the **[CLI Usage Guide](./docs/cli_usage.md)**.

## Contributing

We welcome contributions! Please open an issue or pull request for any changes.

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

For support and questions, please open an issue on the [GitHub repository](https://github.com/chain4energy/lockup-contract/issues).


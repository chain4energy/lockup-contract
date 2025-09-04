# Lockup Contract CLI Usage Guide

This guide provides instructions on how to build, deploy, and interact with the `lockup-contract` using the `c4ed` command-line interface.

## 1. Prerequisites

- You have `rustup` and `cargo` installed.
- You have the `wasm32-unknown-unknown` target installed (`rustup target add wasm32-unknown-unknown`).
- You have `c4ed` installed and configured to connect to a C4E network.

## 2. Build the Contract

First, compile the contract to a WebAssembly (WASM) binary.

```bash
# Navigate to the root of the lockup-contract directory
cd /path/to/lockup-contract

# Compile the contract
cargo wasm
```

This command creates an optimized WASM file located at `artifacts/lockup_contract.wasm`.

## 3. Deploy and Instantiate the Contract

### Step 3.1: Store the WASM code on the blockchain

```bash
# Define shell variables for convenience
export CHAIN_ID="c4e-veles-dev-1"
export NODE="https://rpc-devnet.c4e.io:443/" # Or your preferred RPC node
export FROM_ADDRESS="<your-wallet-name>" # e.g., "validator"

# Store the contract
c4ed tx wasm store artifacts/lockup_contract.wasm \
  --from $FROM_ADDRESS \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

After the transaction is successful, you will get a response containing the `code_id`. Note this `CODE_ID` for the next step.

```bash
export CODE_ID=1 # Replace with your actual code ID
```

### Step 3.2: Instantiate the Contract

Prepare an `instantiate.json` file with the initial configuration.

**`instantiate.json` example:**
```json
{
  "admins": ["c4e1..."],
  "denom": "uc4e",
  "lockup_duration_seconds": 31536000,
  "tier_config": {
    "tier_1_min": "10000000000",
    "tier_2_min": "50000000000",
    "tier_3_min": "100000000000",
    "tier_4_min": "500000000000",
    "tier_4_limit": "1000000000000",
    "tier_1_apr": "0.02",
    "tier_2_apr": "0.035",
    "tier_3_apr": "0.05",
    "tier_4_apr": "0.08",
    "percentage_increase_per_year": "0.01",
    "max_percentage_increase": "0.1"
  }
}
```
*Note: `admins` is a list of addresses. `lockup_duration_seconds` is 1 year. APRs are decimals (e.g., "0.05" is 5%). Amounts are in the smallest unit (e.g., `uc4e`).*

Now, instantiate the contract using the `CODE_ID` from the previous step.

```bash
# Instantiate the contract
c4ed tx wasm instantiate $CODE_ID '$(cat instantiate.json)' \
  --from $FROM_ADDRESS \
  --label "C4E Lockup Contract" \
  --admin "<your-admin-address>" \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

The response will contain the `contract_address`. Export this address for future use.

```bash
export CONTRACT_ADDRESS="c4e1..." # Replace with your new contract address
```

## 4. Interacting with the Contract (Execute Messages)

### 4.1. Deposit Rewards (Admin Only)

Only an address from the `admins` list can fund the reward pool.
This feature will be removed in the future, everyone is able to donate to the Smart Contract's balance using the bank command.

```bash
# Deposit 1,000,000 C4E as rewards
c4ed tx wasm execute $CONTRACT_ADDRESS \
  '{"deposit_rewards":{}}' \
  --from $FROM_ADDRESS \
  --amount 1000000000000uc4e \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

### 4.2. Lock Tokens

A user locks their tokens by sending them with a `lock` message.

```bash
# Lock 10,000 C4E (which qualifies for Tier 1)
c4ed tx wasm execute $CONTRACT_ADDRESS \
  '{"lock":{}}' \
  --from <user-wallet-name> \
  --amount 10000000000uc4e \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

### 4.3. Claim Rewards

The user can claim their accrued rewards at any time.

```bash
c4ed tx wasm execute $CONTRACT_ADDRESS \
  '{"claim_rewards":{}}' \
  --from <user-wallet-name> \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

### 4.4. Unlock Principal

After the lockup period has passed, the user can withdraw their principal and any final rewards.

```bash
c4ed tx wasm execute $CONTRACT_ADDRESS \
  '{"unlock_principal":{}}' \
  --from <user-wallet-name> \
  --gas-prices 0.025uc4e \
  --gas auto \
  --gas-adjustment 1.3 \
  --chain-id $CHAIN_ID \
  --node $NODE \
  -y
```

## 5. Querying the Contract State

### 5.1. Get Contract Configuration

```bash
c4ed query wasm contract-state smart $CONTRACT_ADDRESS \
  '{"get_config":{}}' \
  --node $NODE
```

### 5.2. Get a User's Lockup Details

```bash
# Replace <user-address> with the actual c4e address
c4ed query wasm contract-state smart $CONTRACT_ADDRESS \
  '{"get_lockup":{"address":"<user-address>"}}' \
  --node $NODE
```

### 5.3. Get a User's Claimable Rewards

```bash
c4ed query wasm contract-state smart $CONTRACT_ADDRESS \
  '{"get_claimable_rewards":{"address":"<user-address>"}}' \
  --node $NODE
```

### 5.4. Get All Lockups

```bash
c4ed query wasm contract-state smart $CONTRACT_ADDRESS \
  '{"get_all_lockups":{}}' \
  --node $NODE
```

### 5.5. Check if Reward Funds are Sufficient

```bash
c4ed query wasm contract-state smart $CONTRACT_ADDRESS \
  '{"check_sufficient_funds":{}}' \
  --node $NODE
```
This will return `{"data":{"sufficient":true}}` or `{"data":{"sufficient":false}}`.

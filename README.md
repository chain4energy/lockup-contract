# Lockup Contract

A CosmWasm smart contract implementing time-locked token deposits with tiered rewards and progressive APR increases.

## Contract Architecture

### Initialization

The contract is initialized with the `InstantiateMsg` structure:

```rust
pub struct InstantiateMsg {
    pub admins: Option<Vec<String>>,
    pub denom: String,
    pub lockup_duration_seconds: u64,
    pub tier_config: TierConfig,
}
```

**Parameters:**
- `admins`: Optional list of contract administrator addresses (defaults to deployer if not specified). Multiple admins can be specified for enhanced security and operational flexibility.
- `denom`: Native token denomination for deposits and rewards
- `lockup_duration_seconds`: Fixed lock period in seconds (e.g., 31,536,000 for 1 year)
- `tier_config`: Complete tier configuration structure

### Core Data Structures

#### TierConfig
```rust
pub struct TierConfig {
    pub tier_1_min: Uint128,
    pub tier_2_min: Uint128,
    pub tier_3_min: Uint128,
    pub tier_4_min: Uint128,
    pub tier_4_limit: Uint128,
    pub tier_1_apr: Decimal,
    pub tier_2_apr: Decimal,
    pub tier_3_apr: Decimal,
    pub tier_4_apr: Decimal,
    pub percentage_increase_per_year: Decimal,
    pub max_percentage_increase: Decimal,
}
```

#### Lockup State
```rust
pub struct Lockup {
    pub owner: Addr,
    pub principal_amount: Coin,
    pub unlock_time: Timestamp,
    pub annual_percentage_rate: Decimal,
    pub last_claim_time: Timestamp,
    pub start_time: Timestamp,
}
```

## Core Functions

### Execute Messages

#### Lock Function
```rust
ExecuteMsg::Lock {}
```

**Implementation Logic:**
1. Validates single coin deposit matching configured denomination
2. Checks deposit amount is non-zero and within tier limits
3. Calculates required reserve funds using maximum possible APR for monthly obligations
4. Verifies contract has sufficient funds for projected monthly rewards
5. Determines tier based on deposit amount using `get_tier_for_amount()`
6. For existing lockups: preserves original unlock time and start time, may upgrade tier
7. For new lockups: sets unlock time to current time + lockup duration
8. Stores lockup state in `LOCKUPS` map keyed by user address

**Tier Determination:**
```rust
fn get_tier_for_amount(amount: Uint128, tier_config: &TierConfig) -> u8 {
    match () {
        _ if amount >= tier_config.tier_4_min => 4,
        _ if amount >= tier_config.tier_3_min => 3,
        _ if amount >= tier_config.tier_2_min => 2,
        _ if amount >= tier_config.tier_1_min => 1,
        _ => 0,
    }
}
```

#### Claim Rewards Function
```rust
ExecuteMsg::ClaimRewards {}
```

**Implementation:**
1. Loads user's lockup from storage
2. Calculates rewards using `calculate_rewards()` function
3. Validates rewards amount is non-zero
4. Updates `last_claim_time` to current block time
5. Executes bank transfer to user
6. Principal amount remains locked

#### Unlock Principal Function
```rust
ExecuteMsg::UnlockPrincipal {}
```

**Implementation:**
1. Validates current time >= lockup unlock_time
2. Calculates final pending rewards
3. Removes lockup from storage (irreversible operation)
4. Transfers both principal and final rewards in single bank message

#### Deposit Rewards Function
```rust
ExecuteMsg::DepositRewards {}
```

**Access Control:** Admin-only function (any of the configured admin addresses)
**Purpose:** Allows contract funding for reward obligations

### Query Functions

#### Available Queries
- `GetConfig {}`: Returns contract configuration
- `GetLockup { address }`: Returns specific lockup details
- `GetClaimableRewards { address }`: Calculates current claimable rewards
- `GetAllLockups {}`: Returns all active lockups with metadata
- `GetDepositedRewards {}`: Calculates available reward funds
- `GetSumLockupsAndDeposits {}`: Returns aggregate statistics
- `GetAllRewards {}`: Calculates total yearly reward obligations
- `CheckCoinAvailability {}`: Calculates if the yearly rewards are sufficient for current lockups.

## Reward Calculation Algorithm

### Progressive APR Implementation

The `calculate_rewards()` function implements complex time-based calculations:

1. **Single Year Calculation:** If claim spans single calendar year relative to start_time
2. **Multi-Year Calculation:** Iterates through each year with different effective APRs

**APR Progression Logic:**
- Year 0 (first year): Base APR from tier
- Year 1+: Base APR + (year_index * percentage_increase_per_year)
- Capped at: Base APR + max_percentage_increase

**Time Precision:**
- Uses 31,536,000 seconds per year (365 days)
- Calculates fractional years for partial periods
- Handles leap seconds through block timestamp precision

### Fund Sufficiency Validation

Before accepting new lockups, the contract calculates monthly reward obligations to ensure sufficient reserves. The validation process in `execute_lock()` follows this logic:

```rust
// Check current yearly reward requirements for all existing lockups
let current_required = query_rewards_per_year(deps.as_ref(), &config)?;

// Calculate additional monthly rewards required for this new lockup
let new_apr = get_apr_for_amount(principal_amount.amount, &config.tier_config);
let max_apr = new_apr + config.tier_config.max_percentage_increase;
let principal_decimal = Decimal::from_atomics(principal_amount.amount, 0).unwrap();
let additional_yearly_rewards = principal_decimal * max_apr;
let additional_monthly_rewards = additional_yearly_rewards / Decimal::from_atomics(12u128, 0).unwrap();
let additional_required = additional_monthly_rewards.atomics() / Uint128::new(10u128.pow(18));

let total_required = current_required.amount + additional_required;
```

The `calculate_yearly_rewards_for_lockup()` helper function ensures maximum possible reward calculation:

```rust
fn calculate_yearly_rewards_for_lockup(lockup: &Lockup, tier_config: &TierConfig) -> StdResult<Uint128> {
    let principal_decimal = Decimal::from_atomics(lockup.principal_amount.amount, 0).unwrap();
    let max_apr = lockup.annual_percentage_rate + tier_config.max_percentage_increase;
    let max_yearly_rewards = principal_decimal * max_apr;
    let reward_amount = max_yearly_rewards.atomics() / Uint128::new(10u128.pow(18));
    Ok(reward_amount)
}
```

## Security Considerations

### Access Control
- Admin functions restricted to configured admin addresses (supports multiple admins for enhanced security)
- Multiple admins can be specified during initialization: `admins: ["addr1", "addr2", "addr3"]`
- Any configured admin can execute admin-only functions like `DepositRewards`
- User functions validate sender matches lockup owner
- No upgrade mechanisms implemented

### Fund Management
- Monthly reserve validation prevents accepting deposits without sufficient reward backing
- Available rewards calculated as: total_balance - total_principal - total_pending_rewards
- Maximum APR calculations ensure worst-case scenario coverage with improved capital efficiency

### State Management
- Single lockup per address limitation
- Lockup removal only on successful unlock
- Atomic operations for claim and unlock functions

### Time Handling
- Uses block timestamp for all time calculations
- Handles multi-year reward calculations with year-specific APRs
- Prevents claims when current_time <= last_claim_time

##

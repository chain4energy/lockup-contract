# ChargEra Lockup Contract Functional Requirements

## 1. Core Lockup Mechanics

- **One Lockup Per Wallet**: The system shall enforce a limit of one active lockup per user wallet address. New deposits from an existing user will upgrade their current lockup rather than creating a new one.
- **Configurable Tiers and Limits**:
    - The minimum and maximum deposit amounts, as well as the tier thresholds, shall be defined at the time of contract instantiation.
    - These parameters are immutable and cannot be changed without a contract upgrade. Future adjustments to the tier system will require deploying a new version of the smart contract.
- **Configurable Lockup Duration**: The duration of the lockup period (e.g., one year) shall be set in seconds at the time of contract instantiation and apply to all new lockups.

## 2. Reward System

- **On-Demand Reward Calculation**: Rewards are not continuously tracked in state. Instead, they shall be calculated "on-demand" whenever a user initiates a claim or a query is made for claimable rewards.
- **Continuous Reward Accrual**:
    - Rewards begin to accrue from the moment of the initial deposit (`start_time`).
    - Rewards shall continue to accrue as long as the user's principal deposit remains in the contract, even after the initial `unlock_time` has passed.
    - The accrual of rewards stops permanently only when the user withdraws their principal deposit using the `UnlockPrincipal` function.
- **Anytime Reward Claims**: Users shall be able to claim their accrued rewards at any point in time, both during and after the lockup period, without affecting their principal deposit, provided the contract has sufficient available funds.

## 3. Progressive APR (Long-Term Incentive)

- **Annual APR Increase**: To incentivize long-term holding, the Annual Percentage Rate (APR) for a lockup shall increase by a fixed factor each year, based on the time elapsed since the original `start_time`.
- **Configurable Increase Factor**: The annual increase factor (e.g., 1%) shall be defined at contract instantiation.
- **Maximum APR Limit**: The progressive APR increase shall be capped at a predefined maximum limit to ensure predictable and sustainable reward obligations.

## 4. Funding and Monitoring

- **Admin-Funded Reward Pool**: The pool of C4E tokens used for paying out rewards must be funded by a contract administrator. The contract shall provide an admin-only `DepositRewards` function for this purpose.
- **Reward Pool Monitoring and Insolvency Prevention**:
    - **On Lock**: Before accepting any new deposit, the contract shall perform a `check_sufficient_reward_funds` validation. It will calculate the estimated monthly reward obligation for the new deposit (at its maximum potential APR) and ensure the contract's available reward pool can cover at least one month of the *total* new commitment. This prevents the contract from accepting funds it cannot reasonably reward.
    - **On Claim/Unlock**: Before paying out rewards, the contract shall verify that the `available_rewards` (Total Balance - All Principals - All Accrued Rewards) are greater than or equal to the amount being claimed. This ensures that one user's claim cannot deplete the funds owed to others.
- **Automated Monitoring Script**: A companion script shall be developed to periodically (e.g., daily) execute monitoring queries (`CheckCoinAvailability`, `GetDepositedRewards`).
    - If the script determines that the reward funds will be depleted within a specified timeframe (e.g., two weeks), it shall send a notification to a designated Slack channel.
    - The notification shall be repeated daily until the reward pool is replenished by an administrator.

## 5. Lockup Upgrades

- **In-Lockup Upgrades**: Users shall be able to "upgrade" their lockup at any time *before* their `unlock_time` has passed by depositing additional tokens.
- **Total Deposit Limit**: The upgrade is only permitted if the user's total locked amount (existing principal + new deposit) does not exceed the maximum deposit limit (`tier_4_limit`).
- **Automatic Reward Claim on Upgrade**: Before processing the new deposit, the contract shall automatically calculate and pay out any rewards accrued up to that point. This ensures a clean state for the subsequent APR and principal recalculation.
- **Automatic Tier Recalculation**: Upon a successful upgrade deposit, the contract shall automatically recalculate the user's tier. If the new total principal qualifies for a higher tier, the user's APR will be updated to the new, higher rate.
- **Preservation of Lockup Time**: An upgrade shall not reset the lockup timer. The original `unlock_time` and `start_time` from the initial deposit must be preserved.

## 6. Special Cases

- **C4E Early Bird Booster**: This feature will be handled manually and is not required to be implemented within the smart contract logic (can be determined via the `GetAllLockups` query).

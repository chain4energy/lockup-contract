# ChargEra Lockup Contract Functional Requirements

## 1. Core Lockup Mechanics

- **One Lockup Per Wallet**: The system shall enforce a limit of one active lockup per user wallet address. New deposits from an existing user will upgrade their current lockup rather than creating a new one.
- **Configurable Tiers and Limits**:
    - The minimum and maximum deposit amounts, as well as the tier thresholds, shall be defined at the time of contract instantiation.
    - These parameters are immutable and cannot be changed without a contract upgrade. Future adjustments to the tier system will require deploying a new version of the smart contract.
- **Configurable Lockup Duration**: The duration of the lockup period (e.g., one year) shall be set in seconds at the time of contract instantiation and apply to all new lockups.

## 2. Reward System

- **On-Demand Reward Calculation**: Rewards are not continuously tracked in state. Instead, they shall be calculated "on-demand" whenever a user initiates a claim or a query is made for claimable rewards. The user interface (GUI) will reflect this by fetching and displaying the current reward amount as needed.
- **Continuous Reward Accrual**:
    - Rewards begin to accrue from the moment of the initial deposit.
    - After the lockup period expires, rewards shall continue to accrue as long as the user's principal deposit remains in the contract.
    - The accrual of rewards stops permanently only when the user withdraws their principal deposit using the `UnlockPrincipal` function.
- **Anytime Reward Claims**: Users shall be able to claim their accrued rewards at any point in time, both during and after the lockup period, without affecting their principal deposit.

## 3. Progressive APR (Long-Term Incentive)

- **Annual APR Increase**: To incentivize long-term holding, the Annual Percentage Rate (APR) for a lockup shall increase by a fixed factor each year.
- **Configurable Increase Factor**: The annual increase factor (e.g., 1%) shall be defined at contract instantiation.
- **Maximum APR Limit**: The progressive APR increase shall be capped at a predefined maximum limit to ensure predictable and sustainable reward obligations.

## 4. Funding and Monitoring

- **Admin-Funded Reward Pool**: The pool of C4E tokens used for paying out rewards must be funded by a contract administrator. The contract shall provide an admin-only `DepositRewards` function for this purpose.
- **Reward Pool Monitoring Query**: The contract provides query functions (`CheckCoinAvailability`, `GetDepositedRewards`, `GetAllRewards`) that checks the balance of the reward pool and estimates how long the funds will last based on the current reward commitments automatically and manually.
- **Automated Monitoring Script**: A companion script shall be developed to periodically (e.g., daily) execute the monitoring query.
    - If the script determines that the reward funds will be depleted within a specified timeframe (e.g., two weeks), it shall send a notification to a designated Slack channel.
    - The notification shall be repeated daily until the reward pool is replenished by an administrator.

## 5. Lockup Upgrades

- **In-Lockup Upgrades**: Users shall be able to "upgrade" their lockup at any time by depositing additional tokens.
- **Total Deposit Limit**: The upgrade is only permitted if the user's total locked amount (existing principal + new deposit) does not exceed the maximum deposit limit (`tier_4_limit`).
- **Automatic Tier Recalculation**: Upon a successful upgrade deposit, the contract shall automatically recalculate the user's tier. If the new total principal qualifies for a higher tier, the user's APR will be updated to the new, higher rate.
- **Preservation of Lockup Time**: An upgrade shall not reset the lockup timer. The original `unlock_time` and `start_time` from the initial deposit must be preserved.

## 6. Special Cases

- **C4E Early Bird Booster**: This feature will be handled manually and is not required to be implemented within the smart contract logic (Through `GetAllLockups` query).

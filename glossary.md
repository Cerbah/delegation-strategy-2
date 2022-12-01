## Active epochs
Number of epochs on Solana during which a validator has been active, meaning it has been producing credits for the epoch.


## APR (Annual percentage rate)
Percentage growth of the investment (stake account balance) in case rewards are taken out. We estimate the validators' APY for each epoch based on that epoch's duration, cluster's SOL supply, validators' earned credits and stake.

Let:
- $E$ be epochs per year,
-  $I_Y$ be cluster inflation per year,
- $I_{Taper}$ be inflation taper,
- $1 - I_{E}$ be inflation taper per epoch,
- $C_v$ be $v$th validator's commission,
- $S_{Supply}$ be cluster's total supply of SOL,
- $V_{i_{Credits}}$ be credits earned by $i$th validator,
- $V_{i_{Stake}}$ be $i$th validator's active stake,
- $p_v$ be $v$th validator's share of inflation rewards,
- $R_{v{_Y}}$ be inflation rewards generated by the validator in a year,
- $R_{v_{Staker}}$ be inflation rewards shared with the stakers over a year,

then:

$$p_v = \\frac{V_{v_{Credits}} \\cdot V_{v_{Stake}}}{ \\sum_{i} V_{i_{Credits}} \\cdot V_{i_{Stake}} }$$

$$I_E = \\sqrt[E]{1 - I_{Taper}}$$

$$R_{v{_Y}} = \\sum_{e=0}^{E-1} \\frac{I_Y \\cdot I_E^e \\cdot p_v \\cdot S_{Supply}}{E} = \\frac{I_Y \\cdot p_v \\cdot S_{Supply} \\cdot (I_E^E - 1)}{E \\cdot (I_E - 1)}$$

$$R_{v_{Staker}} = R_{v{_Y}} \cdot (1 - C_v)$$

$$APR_v = \\frac{R_{v_{Staker}}}{V_{v_{Stake}}}$$

Epochs' durations, cluster SOL supply, credits earned by validators, total amount of staked SOL - all of these change throughout the year and therefore it is virtually impossible to accurately project the staking yield. Our calculations are to be considered as mere optimistic estimations.

## APY (Annual percentage yield)
Percentage growth of the investment (stake account balance) over the year given the rewards earned per every epoch are left inside the stake accounts and therefore earn additional rewards. Please note that this is just an estimate (see APR).
Let:
- $APR_v$ be APR of the $v$th validator,
- $E$ be epochs per year,

then:
$$APY_v = (1 + \frac{APR_v}{E})^{E - 1}$$

## Blacklisted validators
List of validators (identity keys) that have been manually blacklisted by Marinade team. A validator can get blacklisted for the following reasons:
- Raising its commission at the very end of epochs to steal the rewards from its stakers 
- Cheating with credits by vote lagging

## Commission
Percentage of the staking rewards that will be taken by the validator for the epoch. Marinade's delegation strategy allows validators to go up to 10% commission, and a bonus to Marinade score is applied for validators running with a lower commission. Check out the [delegation strategy](https://docs.marinade.finance/marinade-protocol/validators) for more details. 

To be detailed (and no link to the docs in the long version) 

## Epoch
In the Solana network, an epoch has a variable time and corresponds to the time a [leader schedule](https://docs.solana.com/terminology#leader-schedule) is valid. An epoch is 432 000 slots long, with a target slot time of 400 ms (which equals approximately 2 days), but the target slot time is not always achieved making the epoch lenght variable. You can follow the evolution of the current and previous epochs on Solana explorers or directly on Marinade. 

## Estimated yearly yield


## Identity
Public key that represents a validator on the Solana network. 

## Marinade rank
Each validator is ranked every epoch by Marinade's delegation strategy, according to the set of criteria it takes into account. Marinade rank represents the relative position of the validator according to Marinade's scoring system.

## Marinade score
Each validator is attributed a score by Marinade's Delegation strategy for the epoch. This score takes into account:
- A
- B
- C
(To be filled) 

## Marinade stake/all active stake

## Node IP
Numerical label that corresponds to the unique IP of the validator (or node). The public IP address of the validator is used for network communication with the validator. It also hints at the validator's geolocation which is an important decentralization factor.

## Node location
Geographical location where the validator (or node) is hosted, deduced by its IP using maxmind DB. 

## Planned stake change
Estimation of the stake to receive or to lose from Marinade for a given validator. 

## Provider company (ASN, ASO)

## Rank active stake

## Skipped slots
In the Solana network, a skipped slot is a slot where the leader did not produce any block, either because it was offline (delinquent) or because the consensus of validators on-chain followed a different fork. Validators should aim to skip as little slots as possible. The leader schedule assigns slots in batches of 4 to different validators and each slot can produce 1 or 0 block, a slot containing 0 block is considered skipped. 

## Stake account distribution
Detailed view of the different stake accounts being delegated to a given validator.

## Stake concentration in city
Amount of staked SOL that is delegated to validators that have the same geographical location. A high concentration of stake in the same city centralizes the network geographically and Marinade strives to distribute the stake accordingly. 

## Stake distribution
?

## Superminority 
In the Solana network, the superminority is composed of the largest validators representing, combined, 33.3..% of all the SOL staked. This set of validators has the power to halt the chain if they were to become delinquent at the same time. Marinade's delegation strategy only delegates outside of the superminority. As a note, it is important to remember that any set of validators representing more than 33.3..% of the total SOL staked on the network have the possibility to halt the network, but the superminority represents the lowest number of validators that meet this criteria. 

## Uptime
Amount of time, in percentage, during which a given validator has been online. Validators have a duty to strive for the highest uptime for possible and only go offline for upgrades. 

## Version
Version of the Solana client used by a given validator.

## Vote account
Public key (??) that is a unique to a given validator and corresponds to the identity that the validator is using to vote to achieve consensus on-chain. 
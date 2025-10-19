use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::Debug,
    hash::Hash,
    iter::{self},
};

/// Trait for game configuration.
pub trait Config<GameT: GameCore>:
    Serialize + for<'de> Deserialize<'de> + Debug + 'static + Default + Clone
{
    /// The error type that can occur during validation of the configuration.
    type ValidationError: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;

    /// Validates the configuration.
    /// # Returns
    /// `Ok(())` if the configuration is valid, otherwise an error.
    fn validate(&self) -> Result<(), Self::ValidationError>;

    /// Returns a list of players that can play the game.
    /// For different configurations, the players list can be different.
    fn get_players(&self) -> Vec<GameT::Player>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerAction<GameT: GameCore> {
    pub player: GameT::Player,
    pub action: GameT::Action,
}

impl<GameT: GameCore> Clone for PlayerAction<GameT> {
    fn clone(&self) -> Self {
        PlayerAction {
            player: self.player.clone(),
            action: self.action.clone(),
        }
    }
}

pub enum InGameEvent<GameT: GameCore> {
    PlayerAction(PlayerAction<GameT>),
    Event(GameT::Event),
}

pub enum Event<GameT: GameCore> {
    InGameEvent(InGameEvent<GameT>),
    GameOver(GameT::Result),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlayerEvent<GameT: GameCore> {
    Event(GameT::PlayerEvent),
    GameOver(GameT::PlayerResult),
}
impl<GameT: GameCore> Clone for PlayerEvent<GameT> {
    fn clone(&self) -> Self {
        match self {
            PlayerEvent::Event(event) => PlayerEvent::Event(event.clone()),
            PlayerEvent::GameOver(result) => PlayerEvent::GameOver(result.clone()),
        }
    }
}

/// Trait for player state in the game.
pub trait PlayerState<GameT: GameCore>:
    Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static
{
    /// Initializes the player state for a given player.
    fn init(config: &GameT::Config, player: GameT::Player) -> GameT::PlayerState;

    /// Returns the player associated with this player state.
    fn get_player(&self) -> GameT::Player;

    /// Checks if the player can take the specified action.
    /// # Arguments
    /// * `action` - The action to check.
    /// # Returns
    /// `Ok(())` if the action can be taken, otherwise an error.
    fn can_take_action(
        &self,
        action: &GameT::Action,
    ) -> Result<(), <GameT::Action as Action<GameT>>::Error>;

    /// Applies an event to the player state.
    /// # Arguments
    /// * `event` - The event to apply. **IS VALID FOR THE CURRENT PLAYER STATE**
    fn apply_event(&mut self, event: &GameT::PlayerEvent);
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "GameT: GameCore"))]
pub struct FullState<GameT: GameCore>
where
    GameT: Debug,
{
    pub config: GameT::Config,
    pub state: GameT::State,
    pub actions_made: Vec<PlayerAction<GameT>>,
}

/// Trait for actions that can be taken in the game.
pub trait Action<GameT: GameCore>:
    Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static
{
    /// The error type that can occur when taking the action.
    type Error: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;
}

/// Game core trait that defines the essential components and behaviors of a game.
pub trait GameCore: Sized + Serialize + for<'de> Deserialize<'de> + Debug {
    /// The configuration of the game. Should contain all necessary parameters to initialize the game.s
    type Config: Config<Self>;
    /// The authoritative state of the game. Should contain all necessary information to represent the game state.
    type State: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;
    /// The action that can be taken in the game. Should represent all possible actions a player can take.
    type Action: Action<Self>;

    /// The player identifier. Should uniquely identify a player in the game. **Should not contain any game state**
    type Player: Serialize + for<'de> Deserialize<'de> + Clone + Copy + Debug + Eq + Hash + 'static;
    /// The player state. Should contain all necessary information about the player's state in the game.
    /// Can be used to hide some information from the game state that is not relevant or visible to the player.
    type PlayerState: PlayerState<Self>;

    /// The event that can occur during the game.
    /// **Events representing played actions and game over are already handled by the game core.**
    type Event: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;
    /// The player event that can occur during the game. Should represent an event that is visible to the player.
    /// **Event representing game over is already handled by the game core.**
    type PlayerEvent: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;

    /// The result of the game. Should represent the authoritative outcome of the game.
    type Result: Serialize + for<'de> Deserialize<'de> + Debug + 'static;
    /// The player result. Should represent the outcome of the game for a specific player.
    type PlayerResult: Serialize + for<'de> Deserialize<'de> + Clone + Debug + 'static;

    /// Initializes the game state based on the provided configuration.
    /// # Arguments
    /// * `config` - The configuration for the game **VALIDATED**
    /// # Returns
    /// The initial state of the game.
    /// # Panics
    /// **SHOULD NOT PANIC**
    fn init(config: &Self::Config) -> Self::State;

    /// Attempts to initialize the game with the provided configuration.
    /// # Arguments
    /// * `config` - The configuration for the game **VALIDATED**
    /// # Returns
    /// A result containing the full state of the game if successful, or an error if initialization fails.
    fn try_init(
        config: &Self::Config,
    ) -> Result<FullState<Self>, <Self::Config as Config<Self>>::ValidationError> {
        config.validate()?;

        Ok(FullState {
            config: config.clone(),
            state: Self::init(config),
            actions_made: Vec::new(),
        })
    }

    /// Takes an action and returns a vector of events that occurred as a result.
    /// # Arguments
    /// * `state` - The current state of the game.
    /// * `player_action` - The action taken by the player **VALID IN THE CURRENT** `state`
    /// # Returns
    /// A vector of events that occurred as a result of the action.
    /// # Panics
    /// **SHOULD NOT PANIC**
    fn take_action(state: &mut Self::State, player_action: PlayerAction<Self>) -> Vec<Self::Event>;

    /// Checks if the game is over and returns the result if it is.
    fn check_game_over(state: &Self::State) -> Option<Self::Result>;

    /// Derives a player event from the game state for the player.
    /// # Arguments
    /// * `state` - The current state of the game.
    /// * `player` - The player for whom the event is being derived.
    /// * `event` - The event that occurred in the game.
    /// # Returns
    /// An optional player event if it can be derived (it is visible for the player in some form), otherwise `None`.
    fn derive_player_event(
        state: &Self::State,
        player: &Self::Player,
        event: &InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent>;

    /// Derives a player result from the game state for the player.
    /// # Arguments
    /// * `state` - The current state of the game.
    /// * `player` - The player for whom the result is being derived.
    /// * `result` - The result of the game.
    /// # Returns
    /// The player result derived from the game state.
    fn derive_player_result(
        state: &Self::State,
        player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult;

    /// Builds a map of player events from the game state and game events.
    /// # Arguments
    /// * `game_state` - The full state of the game.
    /// * `game_events` - The events that occurred in the game.
    /// # Returns
    /// A map of player events where each player is associated with a vector of their events.
    fn build_player_events_map(
        game_state: &FullState<Self>,
        game_events: &[Event<Self>],
    ) -> HashMap<Self::Player, Vec<PlayerEvent<Self>>> {
        let mut player_events_map = HashMap::<Self::Player, Vec<PlayerEvent<Self>>>::new();
        let players = game_state.config.get_players();

        players.iter().for_each(|player| {
            player_events_map.insert(*player, Vec::new());

            game_events
                .iter()
                .filter_map(|event| match event {
                    Event::InGameEvent(in_game_event) => {
                        Self::derive_player_event(&game_state.state, player, in_game_event)
                            .and_then(|player_event| Some(PlayerEvent::Event(player_event)))
                    }
                    Event::GameOver(result) => {
                        let player_result =
                            Self::derive_player_result(&game_state.state, player, result);
                        Some(PlayerEvent::GameOver(player_result))
                    }
                })
                .for_each(|player_event| {
                    player_events_map
                        .get_mut(&player)
                        .expect("Player should exist in the map")
                        .push(player_event);
                });
        });

        player_events_map
    }

    /// Applies an action to the game state and returns a map of player events.
    /// # Arguments
    /// * `game_state` - The current full state of the game. **WILL BE MUTATED**
    /// * `player_action` - The action taken by the player.
    /// * `player_state` - The state of the player taking the action.
    /// # Returns
    /// A result containing a map of player events if the action was successfully applied,
    /// or an error if the action could not be applied.
    fn apply_action(
        game_state: &mut FullState<Self>,
        player_action: PlayerAction<Self>,
        player_state: &Self::PlayerState,
    ) -> Result<HashMap<Self::Player, Vec<PlayerEvent<Self>>>, <Self::Action as Action<Self>>::Error>
    {
        player_state.can_take_action(&player_action.action)?;

        game_state.actions_made.push(player_action.clone());

        let events = Self::take_action(&mut game_state.state, player_action.clone());
        let game_events: Vec<Event<Self>> =
            iter::once(Event::InGameEvent(InGameEvent::PlayerAction(player_action)))
                .chain(
                    events
                        .into_iter()
                        .map(|event| Event::InGameEvent(InGameEvent::Event(event))),
                )
                .chain(
                    Self::check_game_over(&game_state.state).map(|result| Event::GameOver(result)),
                )
                .collect();

        Ok(Self::build_player_events_map(game_state, &game_events))
    }
}

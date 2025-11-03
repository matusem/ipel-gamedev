use std::collections::{HashMap, VecDeque};

use game::{Config as GameConfig, GameCore};
use itertools::{self, Itertools};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Zolik;
impl Zolik {
    fn get_deck() -> Vec<Card> {
        let french_deck = Suit::all_suits()
            .collect::<Vec<_>>()
            .into_iter()
            .cartesian_product(Rank::all_ranks().collect::<Vec<_>>())
            .map(|(suit, rank)| Card::Card(suit, rank))
            .chain([Card::Joker; 2]);
        return french_deck.clone().chain(french_deck.clone()).collect();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Player(u8);

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct Random(usize);
impl Random {
    fn new(seed: usize) -> Random {
        Random(seed)
    }

    fn get(mut self) -> usize {
        self.0 = self.0.wrapping_mul(1103515245).wrapping_add(12345) % (1 << 31);
        self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Config {
    num_players: u8,
    seed: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum ConfigValidationError {
    InvalidNumPlayers,
}

impl GameConfig<Zolik> for Config {
    type ValidationError = ConfigValidationError;

    fn validate(&self) -> Result<(), Self::ValidationError> {
        if self.num_players < 2 {
            return Err(ConfigValidationError::InvalidNumPlayers);
        }
        return Ok(());
    }

    fn get_players(&self) -> Vec<Player> {
        return (0..=self.num_players - 1).map(|i| Player(i)).collect();
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
enum Suit {
    Hearts,
    Diamonds,
    Clubs,
    Spades,
}
impl Suit {
    fn all_suits() -> impl Iterator<Item = Suit> {
        return [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades].into_iter();
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
struct Rank(u8); // 2-14 (2-10, J, Q, K, A)
impl Rank {
    fn all_ranks() -> impl Iterator<Item = Rank> {
        return (2..14).map(|num| Rank(num));
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
enum Card {
    Joker,
    Card(Suit, Rank),
}
#[derive(Serialize, Deserialize, Clone, Debug)]
enum CardSetType {
    Straight,
    Suits,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct CardSet {
    cards: Vec<Card>,
    set_type: CardSetType, // maybe not needed
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum TurnPart {
    Draw,
    TheRestOfTheFuckingOwl,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlayingState {
    player_turn: Player,
    turn_part: TurnPart,
    player_hands: HashMap<Player, Vec<Card>>,
    player_sets: HashMap<Player, Vec<CardSet>>,
    deck: Vec<Card>,
    discard_pile: Vec<Card>,
    specialna_karta_co_je_na_spodku_decku: Card,
    rng: Random,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct SetupState {
    player_turn: Player,
    players: Vec<Player>,
    deck: Vec<Card>,
    rng: Random,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum State {
    Setup(SetupState),
    Playing(PlayingState),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PlayerState {
    state_owner: Player,
    player_turn: Player,
    hand: Vec<Card>,
    player_sets: HashMap<Player, Vec<CardSet>>,
    cards_left_in_deck: usize,
    discard_pile_top: Card,
    specialna_karta_co_je_na_spodku_decku: Card,
}

impl game::PlayerState<Zolik> for PlayerState {
    fn init(
        config: &<Zolik as GameCore>::Config,
        player: <Zolik as GameCore>::Player,
    ) -> <Zolik as GameCore>::PlayerState {
        todo!();
    }

    fn get_player(&self) -> <Zolik as GameCore>::Player {
        return self.state_owner;
    }

    fn can_take_action(
        &self,
        action: &<Zolik as GameCore>::Action,
    ) -> Result<(), <<Zolik as GameCore>::Action as game::Action<Zolik>>::Error> {
        todo!()
    }

    fn apply_event(&mut self, event: &<Zolik as GameCore>::PlayerEvent) {
        todo!()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Action {
    CutTheDeck(usize),
    DrawFromDeck,
    DrawFromDiscard,
    InitialPlaySets(Vec<CardSet>),
    PlaySet(CardSet),
    AddToSet(Card, CardSet),
    Discard(Card),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum ActionError {
    NotYourTurn,
    MustDrawFirst,
    CannotDrawAgain,
    CannotDrawFromDiscard,
    // TODO
}
impl game::Action<Zolik> for Action {
    type Error = ActionError;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PlayerPoints {
    player_points: HashMap<Player, usize>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
enum GameResult {
    PlayerWon(Player, PlayerPoints),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct SetupCompleteData {
    revealed_cards: [Card; 3],
    number_of_jokers: u8,
    bottom_card: Card,
    updated_deck: Vec<Card>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Event {
    SetupComplete(SetupCompleteData),
}

impl GameCore for Zolik {
    type Config = Config;

    type State = State;

    type Action = Action;

    type Player = Player;

    type PlayerState = PlayerState;

    type Event = Event;

    type PlayerEvent = ();

    type Result = GameResult;

    type PlayerResult = GameResult;

    fn init(config: &Self::Config) -> Self::State {
        let players = config.get_players();
        let ordered_deck = Self::get_deck();
        let rng = Random(config.seed);
        let random_deck = ordered_deck
            .into_iter()
            .map(|card| (rng.get(), card))
            .sorted_by(|a, b| Ord::cmp(&a.0, &b.0))
            .map(|(_, card)| card);
        return State::Setup(SetupState {
            player_turn: players[0],
            deck: random_deck.collect(),
            players,
            rng,
        });
    }

    fn take_action(
        state: &mut Self::State,
        player_action: game::PlayerAction<Self>,
    ) -> Vec<Self::Event> {
        match state {
            State::Setup(state) => match player_action.action {
                Action::CutTheDeck(position) => {
                    // top of the deck is index 0, so start_slice is the part the user took from the deck
                    let (start_slice, end_slice) = state.deck.split_at(position);
                    // we check if there is joker in bottom 3 cards
                    let (rest, checked_cards) = start_slice.split_at(start_slice.len() - 3);
                    let (non_jokers, jokers): (Vec<Card>, Vec<Card>) =
                        checked_cards.iter().partition(|card| match card {
                            Card::Joker => false,
                            _ => true,
                        });

                    // take non joker cards and return them
                    let start_slice: Vec<&Card> = rest.iter().chain(non_jokers.iter()).collect();

                    // put the top card at the bottom
                    let specialna_karta_co_je_na_spodku_decku = start_slice[0].clone();
                    let start_slice: Vec<Card> = start_slice.into_iter().skip(1).copied().collect();
                    let mut deck: VecDeque<Card> = end_slice
                        .iter()
                        .chain(start_slice.iter())
                        .copied()
                        .collect();

                    let mut player_hands: HashMap<Player, Vec<Card>> = HashMap::new();
                    for player in &state.players {
                        if player == &state.player_turn {
                            player_hands.insert(*player, jokers.clone());
                        } else {
                            player_hands.insert(*player, vec![]);
                        }
                    }

                    for player_hand in player_hands.iter_mut() {
                        while player_hand.1.len() < 14 {
                            match deck.pop_front() {
                                Some(card) => player_hand.1.push(card),
                                None => unreachable!(),
                            };
                        }
                    }
                }
                _ => unreachable!(),
            },
            State::Playing(state) => {
                todo!()
            }
        };
        todo!()
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        todo!()
    }

    fn derive_player_event(
        state: &Self::State,
        player: &Self::Player,
        event: &game::InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        todo!()
    }

    fn derive_player_result(
        state: &Self::State,
        player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn init_game_state() {
        let config = Config {
            num_players: 4,
            seed: 12345,
        };
        let initial_state = Zolik::init(&config);
        println!("{:?}", initial_state);
    }
}

use std::{
    cmp::min,
    collections::{HashMap, VecDeque},
};

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

    fn get(&mut self) -> usize {
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
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
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
struct Rank(u8); // 2-14 (2-10, J, Q, K, A)
impl Rank {
    fn all_ranks() -> impl Iterator<Item = Rank> {
        return (2..14).map(|num| Rank(num));
    }
}
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
enum Card {
    Joker,
    Card(Suit, Rank),
}
impl Card {
    fn end_game_value(&self) -> u8 {
        match self {
            Card::Joker => 50,
            Card::Card(_, rank) => min(rank.0, 10),
        }
    }
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
enum TurnPart {
    Draw,
    TheRestOfTheFuckingOwl,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct PlayingState {
    player_turn: Player,
    players: Vec<Player>,
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
enum PlayerState {
    Setup(SetupPlayerState),
    Playing(PlayingPlayerState),
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct SetupPlayerState {
    state_owner: Player,
    player_turn: Player,
    players: Vec<Player>,
    deck_size: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct PlayingPlayerState {
    state_owner: Player,
    player_turn: Player,
    hand: Vec<Card>,
    player_sets: HashMap<Player, Vec<CardSet>>,
    cards_left_in_deck: usize,
    discard_pile_top: Option<Card>,
    specialna_karta_co_je_na_spodku_decku: Card,
}

impl game::PlayerState<Zolik> for PlayerState {
    fn init(
        config: &<Zolik as GameCore>::Config,
        player: <Zolik as GameCore>::Player,
    ) -> <Zolik as GameCore>::PlayerState {
        return PlayerState::Setup(SetupPlayerState {
            state_owner: player,
            player_turn: config.get_players()[0],
            players: config.get_players(),
            deck_size: Zolik::get_deck().len(),
        });
    }

    fn get_player(&self) -> <Zolik as GameCore>::Player {
        return match self {
            PlayerState::Setup(state) => state.state_owner,
            PlayerState::Playing(state) => state.state_owner,
        };
    }

    fn can_take_action(
        &self,
        action: &<Zolik as GameCore>::Action,
    ) -> Result<(), <<Zolik as GameCore>::Action as game::Action<Zolik>>::Error> {
        match action {
            Action::CutTheDeck(position) => match self {
                PlayerState::Setup(state) => {
                    if state.player_turn != state.state_owner {
                        return Err(ActionError::NotYourTurn);
                    }
                    if *position < 3 || *position > state.deck_size - 3 {
                        return Err(ActionError::InvalidCutPosition);
                    }
                    Ok(())
                }
                _ => return Err(ActionError::CannotCutAgain),
            },
            _ => {
                todo!()
            }
        }
    }

    fn apply_event(&mut self, event: &<Zolik as GameCore>::PlayerEvent) {
        match event {
            PlayerEvent::Event(player_event) => match player_event {
                Event::SetupComplete(data) => match self {
                    PlayerState::Setup(state) => {
                        *self = PlayerState::Playing(PlayingPlayerState {
                            state_owner: state.state_owner,
                            player_turn: state.players[1],
                            hand: data.player_hands.get(&state.state_owner).unwrap().to_vec(),
                            player_sets: state
                                .players
                                .iter()
                                .map(|player| {
                                    (
                                        *player,
                                        vec![], // no sets at the start
                                    )
                                })
                                .collect(),
                            cards_left_in_deck: data.updated_deck.len(),
                            discard_pile_top: None,
                            specialna_karta_co_je_na_spodku_decku: data.bottom_card,
                        });
                    }
                    _ => unreachable!(),
                },
                Event::CardDrawn(data) => match self {
                    PlayerState::Playing(state) => {
                        if state.state_owner == data.player {
                            state.hand = data.hand.clone();
                            state.cards_left_in_deck = data.cards_left_in_deck;
                        }
                    }
                    _ => unreachable!(),
                },
                Event::CardDiscarded(data) => match self {
                    PlayerState::Playing(state) => {
                        if state.state_owner == data.player {
                            state.hand = data.hand.clone();
                        }
                        state.discard_pile_top = Some(data.card);
                    }
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
            PlayerEvent::Action(_) => {
                // Actions do not modify player state directly
            }
        }
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
    Discard(usize),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum ActionError {
    //Setup
    CannotCutAgain,
    InvalidCutPosition,
    //Playing
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
    bottom_card: Card,
    updated_deck: Vec<Card>,
    player_hands: HashMap<Player, Vec<Card>>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CardDrawnData {
    player: Player,
    card: Card,
    hand: Vec<Card>,
    cards_left_in_deck: usize,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CardDiscardedData {
    player: Player,
    card: Card,
    hand: Vec<Card>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Event {
    SetupComplete(SetupCompleteData),
    CardDrawn(CardDrawnData),
    CardDiscarded(CardDiscardedData),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum PlayerEvent {
    Event(Event),
    Action(Action),
}

impl GameCore for Zolik {
    type Config = Config;

    type State = State;

    type Action = Action;

    type Player = Player;

    type PlayerState = PlayerState;

    type Event = Event;

    type PlayerEvent = PlayerEvent;

    type Result = GameResult;

    type PlayerResult = GameResult;

    fn init(config: &Self::Config) -> Self::State {
        let players = config.get_players();
        let ordered_deck = Self::get_deck();
        let mut rng = Random::new(config.seed);
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
            State::Setup(setup_state) => match player_action.action {
                Action::CutTheDeck(position) => {
                    // top of the deck is index 0, so start_slice is the part the user took from the deck
                    let (start_slice, end_slice) = setup_state.deck.split_at(position);
                    // we check if there is joker in bottom 3 cards
                    let (rest, checked_cards) = start_slice.split_at(start_slice.len() - 3);
                    let checked_cards: Vec<Card> = checked_cards.to_vec();
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
                    for player in &setup_state.players {
                        if player == &setup_state.player_turn {
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
                    *state = State::Playing(PlayingState {
                        player_turn: setup_state.players[1],
                        players: setup_state.players.clone(),
                        turn_part: TurnPart::Draw,
                        player_hands: player_hands.clone(),
                        player_sets: setup_state
                            .players
                            .iter()
                            .map(|player| (*player, vec![]))
                            .collect(),
                        deck: deck.iter().copied().collect(),
                        discard_pile: vec![],
                        specialna_karta_co_je_na_spodku_decku,
                        rng: setup_state.rng.clone(),
                    });
                    return vec![Event::SetupComplete(SetupCompleteData {
                        revealed_cards: checked_cards.try_into().unwrap(),
                        bottom_card: specialna_karta_co_je_na_spodku_decku,
                        updated_deck: deck.into_iter().collect(),
                        player_hands,
                    })];
                }
                _ => unreachable!(),
            },
            State::Playing(state) => match player_action.action {
                Action::DrawFromDeck => {
                    state.turn_part = TurnPart::TheRestOfTheFuckingOwl;
                    let drawn_card = state.deck.pop().unwrap();
                    state
                        .player_hands
                        .get_mut(&player_action.player)
                        .unwrap()
                        .push(drawn_card);
                    return vec![Event::CardDrawn(CardDrawnData {
                        player: player_action.player,
                        card: drawn_card,
                        hand: state
                            .player_hands
                            .get(&player_action.player)
                            .unwrap()
                            .to_vec(),
                        cards_left_in_deck: state.deck.len(),
                    })];
                }
                Action::Discard(card_index) => {
                    let player_hand = state.player_hands.get_mut(&player_action.player).unwrap();
                    let discarded_card = player_hand.remove(card_index);
                    state.discard_pile.push(discarded_card);
                    // advance turn
                    let current_player_index = state
                        .players
                        .iter()
                        .position(|p| *p == player_action.player)
                        .unwrap();
                    let next_player_index = (current_player_index + 1) % state.players.len();
                    state.player_turn = state.players[next_player_index];
                    state.turn_part = TurnPart::Draw;
                    return vec![Event::CardDiscarded(CardDiscardedData {
                        player: player_action.player,
                        card: discarded_card,
                        hand: player_hand.to_vec(),
                    })];
                }
                _ => {
                    todo!()
                }
            },
        };
    }

    fn check_game_over(state: &Self::State) -> Option<Self::Result> {
        match state {
            State::Playing(playing_state) => {
                for (player, hand) in playing_state.player_hands.iter() {
                    if hand.is_empty() {
                        return Some(GameResult::PlayerWon(
                            player.clone(),
                            PlayerPoints {
                                player_points: playing_state
                                    .player_hands
                                    .keys()
                                    .map(|p| {
                                        (
                                            p.clone(),
                                            playing_state.player_hands[p]
                                                .iter()
                                                .map(|c| c.end_game_value() as usize)
                                                .sum(),
                                        )
                                    })
                                    .collect(),
                            },
                        ));
                    }
                }
                return None;
            }
            _ => return None,
        }
    }

    fn derive_player_event(
        state: &Self::State,
        player: &Self::Player,
        event: &game::InGameEvent<Self>,
    ) -> Option<Self::PlayerEvent> {
        match event {
            game::InGameEvent::PlayerAction(action) => {
                return Some(PlayerEvent::Action(action.action.clone()));
            }
            game::InGameEvent::Event(event) => return Some(PlayerEvent::Event(event.clone())),
        }
    }

    fn derive_player_result(
        state: &Self::State,
        player: &Self::Player,
        result: &Self::Result,
    ) -> Self::PlayerResult {
        result.clone()
    }
}

#[cfg(test)]
mod tests {
    use game::PlayerAction;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    fn get_playing_state_for_tests(deck: &Vec<Card>) -> State {
        let config = Config {
            num_players: 4,
            seed: 12345,
        };
        State::Playing(PlayingState {
            player_turn: Player(0),
            players: config.get_players(),
            turn_part: TurnPart::Draw,
            player_hands: HashMap::from([
                (
                    Player(0),
                    vec![
                        Card::Card(Suit::Hearts, Rank(2)),
                        Card::Card(Suit::Hearts, Rank(3)),
                        Card::Card(Suit::Hearts, Rank(4)),
                        Card::Card(Suit::Diamonds, Rank(3)),
                    ],
                ),
                (
                    Player(1),
                    vec![Card::Joker, Card::Card(Suit::Spades, Rank(14))],
                ),
                (Player(2), vec![Card::Card(Suit::Clubs, Rank(10))]),
                (Player(3), vec![Card::Card(Suit::Hearts, Rank(7))]),
            ]),
            player_sets: config.get_players().iter().map(|p| (*p, vec![])).collect(),
            deck: deck.clone(),
            discard_pile: vec![Card::Card(Suit::Hearts, Rank(5))],
            specialna_karta_co_je_na_spodku_decku: Card::Card(Suit::Spades, Rank(14)),
            rng: Random::new(12345),
        })
    }

    #[test]
    fn init_game_state() {
        let config = Config {
            num_players: 4,
            seed: 12345,
        };
        let initial_state = Zolik::init(&config);
        match initial_state {
            State::Setup(state) => {
                assert_eq!(state.players.len(), 4);
                assert_eq!(state.deck.len(), Zolik::get_deck().len());
                assert_eq!(&state.player_turn, state.players.get(0).unwrap());
                assert_eq!(state.rng.0, 12345);
            }
            _ => panic!("Initial state should be Setup"),
        };
    }
    #[test]
    fn random_works() {
        let mut rng = Random::new(12345);
        let first = rng.get();
        let second = rng.get();
        let third = rng.get();
        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
    }

    #[test]
    fn setup_completes_successfully() {
        let config = Config {
            num_players: 4,
            seed: 12345,
        };
        let mut state = Zolik::init(&config);
        match &mut state {
            State::Setup(setup_state) => {
                let cut_position = 10;
                let player_action = game::PlayerAction {
                    player: setup_state.player_turn,
                    action: Action::CutTheDeck(cut_position),
                };
                let events = Zolik::take_action(&mut state, player_action);
                // match state {
                //     State::Playing(playing_state) => {
                //         assert_eq!(playing_state.player_turn, setup_state.players[1]);
                //         assert_eq!(playing_state.turn_part, TurnPart::Draw);
                //     }
                //     _ => panic!("State should be Playing after cutting the deck"),
                // }
                assert_eq!(events.len(), 1);
                match &events[0] {
                    Event::SetupComplete(data) => {
                        // there must be 3 revealed cards
                        assert_eq!(data.revealed_cards.len(), 3);
                        // there must be exactly as many player hands as players
                        assert!(data.player_hands.len() == config.num_players.into());
                        // each player must have 14 cards
                        for hand in data.player_hands.values() {
                            assert_eq!(hand.len(), 14);
                        }
                        // the updated deck must have correct number of cards
                        // = total cards - bottom card - cards in hands
                        assert_eq!(
                            data.updated_deck.len(),
                            Zolik::get_deck().len()
                                - 1 // bottom card
                                - (data
                                    .player_hands
                                    .values()
                                    .map(|hand| hand.len())
                                    .sum::<usize>())
                        );
                        // there must be at most 3 jokers in revealed cards (in case we use more decks, not implemented yet)
                        assert!(
                            data.revealed_cards
                                .iter()
                                .filter(|c| **c == Card::Joker)
                                .count()
                                <= 3
                        );
                        assert!(data.player_hands.values().all(|hand| hand.len() == 14));
                    }
                    _ => panic!("Expected only SetupComplete event"),
                }
            }
            _ => panic!("State should be Setup"),
        }
    }
    #[test]
    fn player_can_draw_card() {
        let ordered_deck = Zolik::get_deck();
        let top_card = ordered_deck.last().unwrap();
        let mut state = get_playing_state_for_tests(&ordered_deck);
        let player_action = PlayerAction {
            player: Player(0),
            action: Action::DrawFromDeck,
        };
        let events = Zolik::take_action(&mut state, player_action);
        match events[0] {
            Event::CardDrawn(ref data) => {
                assert_eq!(data.player, Player(0));
                assert_eq!(data.card, *top_card);
                assert_eq!(data.hand.len(), 5); // player had 4 cards before
                assert!(data.hand.iter().any(|c| *c == *top_card));
                assert_eq!(data.cards_left_in_deck, ordered_deck.len() - 1);
            }
            _ => panic!("Expected CardDrawn event"),
        }
    }
    #[test]
    fn player_can_discard_card() {
        let ordered_deck = Zolik::get_deck();
        let top_card = ordered_deck.last().unwrap();
        let mut state = get_playing_state_for_tests(&ordered_deck);
        // first draw a card
        let player_action = PlayerAction {
            player: Player(0),
            action: Action::DrawFromDeck,
        };
        Zolik::take_action(&mut state, player_action);
        // then discard a card
        let player_action = PlayerAction {
            player: Player(0),
            action: Action::Discard(4), // discard the card we just drew
        };
        let events = Zolik::take_action(&mut state, player_action);
        match events[0] {
            Event::CardDiscarded(ref data) => {
                assert_eq!(data.player, Player(0));
                assert_eq!(data.hand.len(), 4); // player had 5 cards before
                assert_eq!(data.card, *top_card);
                assert!(data.hand.iter().all(|c| *c != *top_card));
            }
            _ => panic!("Expected CardDiscarded event"),
        }
    }
}

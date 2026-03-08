use core::f32;
use std::{cell::Cell, iter, rc::Rc};

use macroquad::rand::ChooseRandom;
use rand::{random_bool, random_range, Rng};

use crate::{
    core::{
        distance_between, sq_distance_between, Ability, AbilityId, AbilityTarget, Action,
        ActionReach, ActionTarget, AttackEnhancement, BaseAction, Character, CharacterId,
        Condition, CoreGame, HandType, OnAttackedReaction, OnHitReaction, Position, Range,
        CENTER_MELEE_RANGE_SQUARED,
    },
    data::{HULDRA_HEAL, HULDRA_INFLICT_HORRORS, HULDRA_INFLICT_WOUNDS, INFLICT_WOUNDS},
    pathfind::{Occupation, Path, PathfindGrid},
    util::{adjacent_cells, are_entities_within_melee, line_visitor, CustomShuffle},
};

#[derive(Debug, Clone)]
pub enum BotBehaviour {
    Normal,
    Huldra(HuldraBehaviour),
    Fighter(FighterBehaviour),
}

#[derive(Debug, Clone, Default)]
pub struct HuldraBehaviour {
    saved_goal: Cell<Option<(BotAction, Option<CharacterId>)>>,
    last_action: Cell<Option<AbilityId>>,
}

impl HuldraBehaviour {
    fn run(&self, game: &CoreGame) -> Option<Action> {
        let action: (BotAction, Option<Rc<Character>>);

        let bot = game.active_character();

        let mut rng = rand::rng();

        let is_healing_warranted = game
            .enemies()
            .any(|char| char.health.current() < char.health.max() - 5);

        let are_all_players_bleeding = game.player_characters().all(|ch| ch.is_bleeding());

        let heal = BotAction::SingleFriendlyTarget(HULDRA_HEAL);
        let inflict_wounds = BotAction::SingleEnemyTarget(HULDRA_INFLICT_WOUNDS);
        let inflict_horrors = BotAction::SingleEnemyTarget(HULDRA_INFLICT_HORRORS);

        if let Some((saved_action, saved_target)) = self.saved_goal.get() {
            action = (
                saved_action,
                saved_target.map(|id| Rc::clone(game.characters.get_rc(id))),
            );
            self.saved_goal.set(None);
        } else if self.last_action.get() != Some(AbilityId::MagiHeal)
            && is_healing_warranted
            && rng.random_bool(0.7)
        {
            let target: &Rc<Character> = game
                .enemies()
                .min_by(|a, b| a.health.ratio().total_cmp(&b.health.ratio()))
                .unwrap();

            action = (heal, Some(Rc::clone(&target)));
            dbg!("NEW Huldra HEAL GOAL: {:?}", target.id());
        } else if self.last_action.get() != Some(AbilityId::MagiInflictWounds)
            && !are_all_players_bleeding
            && rng.random_bool(0.8)
        {
            let mut non_bleeding_player_chars: Vec<&Rc<Character>> = game
                .player_characters()
                .filter(|ch| !ch.is_bleeding())
                .collect();

            non_bleeding_player_chars.sort_by_key(|ch| {
                let range = HULDRA_INFLICT_WOUNDS.target.range(&[]).unwrap();
                let distance_to = find_path(game, bot, ch, range)
                    .map(|p| p.total_distance)
                    .unwrap_or(f32::MAX);

                // convert from f32 to make it sortable
                (distance_to * 10.0) as u32
            });

            let target = non_bleeding_player_chars[0];

            dbg!("NEW Huldra WOUND GOAL: {:?}", target.id());

            action = (inflict_wounds, Some(Rc::clone(&target)));
        } else {
            let player_chars: Vec<&Rc<Character>> = game.player_characters().collect();
            let target = player_chars[rng.random_range(0..player_chars.len())];

            dbg!("NEW Huldra HORROR GOAL: {:?}", target.id());

            action = (inflict_horrors, Some(Rc::clone(&target)));
        }

        let goal = BotGoal {
            action,
            fallback_actions: vec![inflict_horrors, inflict_wounds, BotAction::Attack, heal],
        };

        let chosen_action = pursue_goal(game, goal.clone());

        match &chosen_action {
            Some(action) => {
                match action {
                    Action::UseAbility { ability, .. } => {
                        self.last_action.set(Some(ability.id));
                    }
                    Action::Move { .. } => {
                        // We probably chose movement to get into range, so we should stick to the same action
                        self.saved_goal
                            .set(Some((goal.action.0, goal.action.1.map(|ch| ch.id()))));
                    }
                    _ => {}
                }
            }
            None => {}
        }

        chosen_action
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenericBehaviour {
    current_goal: Cell<Option<(BotAction, Option<CharacterId>)>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FighterBehaviour {
    target_selection: EnemyTargetSelection,
}

impl FighterBehaviour {
    fn get_goal(&self, game: &CoreGame) -> BotGoal {
        let (player_chars, target_id) = self.target_selection.run(game);
        let bot = game.characters.get_rc(game.active_character_id);
        let target = player_chars.iter().find(|ch| ch.id() == target_id).unwrap();
        let target = Rc::clone(target);

        let candidates = candidate_actions(bot);

        let action = candidates[0];
        let action = match action {
            BotAction::Attack => (action, Some(target)),
            BotAction::SingleEnemyTarget(..) => (action, Some(target)),
            // TODO should not only target self
            BotAction::SingleFriendlyTarget(..) => (action, Some(Rc::clone(bot))),
            BotAction::NonTarget(..) => (action, None),
        };

        BotGoal {
            action,
            fallback_actions: candidates,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnemyTargetSelection {
    current_target: Cell<Option<CharacterId>>,
    chance_of_switching_target: Cell<f32>,
}

impl EnemyTargetSelection {
    fn run<'a>(&self, game: &'a CoreGame) -> (Vec<&'a Rc<Character>>, CharacterId) {
        let bot = game.active_character();

        let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();

        if let Some(target_id) = self.current_target.get() {
            if !player_chars.iter().any(|ch| ch.id() == target_id) {
                // Player char must have died. Force a target switch.
                self.current_target.set(None);
            }
        }

        if random_bool(0.5) {
            println!("Sort player chars by proximity");
            player_chars.sort_by_key(|ch| {
                let distance_to = find_path_to_attack_target(game, bot, ch)
                    .map(|p| p.total_distance)
                    .unwrap_or(f32::MAX);

                // convert from f32 to make it sortable
                (distance_to * 10.0) as u32
            });
        } else {
            println!("Shuffle player chars");
            CustomShuffle::shuffle(&mut player_chars);
        }

        if self.current_target.get().is_none() {
            // TODO: this panics if all player chars have died
            self.current_target.set(Some(player_chars[0].id()));
        }

        let mut target_id = self.current_target.get().unwrap();

        let chance_switch_target = self.chance_of_switching_target.get();
        dbg!(chance_switch_target);
        let should_switch_target = random_range(0.0..1.0) < chance_switch_target;
        dbg!(should_switch_target);
        if should_switch_target {
            self.chance_of_switching_target.set(0.0);
        } else if self.chance_of_switching_target.get() == 0.0 {
            // Make it very rare to switch target immediately after acquiring it
            self.chance_of_switching_target.set(0.01);
        } else {
            // After that, the chance increases steadily
            self.chance_of_switching_target
                .set(chance_switch_target + 0.1);
        }

        if should_switch_target {
            println!("bot should try switching target");
            if let Some(new_target) = player_chars.iter().find(|ch| ch.id() != target_id) {
                println!("switching to new target?: {}", new_target.id());

                if find_path_to_attack_target(game, bot, new_target).is_some() {
                    println!("Yes, there's a path to it ");
                    self.current_target.set(Some(new_target.id()));
                    target_id = new_target.id();
                }
            }
        } else {
            println!("bot sticks with current target: {:?}", self.current_target);
        }

        (player_chars, target_id)
    }
}

const EXPLORATION_RANGE: f32 = 60.0;

pub fn bot_choose_action(game: &CoreGame) -> Option<Action> {
    let character = game.active_character();
    assert!(!character.player_controlled());

    dbg!("BOT CHOOSING ACTION ...");

    let result = match character.kind.unwrap_bot_behaviour() {
        BotBehaviour::Normal => run_normal_behaviour(game),
        BotBehaviour::Huldra(huldra) => huldra.run(game),
        BotBehaviour::Fighter(fighter) => pursue_goal(game, fighter.get_goal(game)),
    };
    println!("Bot chose: {:?}", result);

    result
}

#[derive(Clone)]
struct BotGoal {
    action: (BotAction, Option<Rc<Character>>),
    fallback_actions: Vec<BotAction>,
}

impl std::fmt::Debug for BotGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BotGoal")
            .field(
                "action",
                &(
                    self.action.0,
                    self.action.1.as_ref().map(|ch| (ch.name, ch.id())),
                ),
            )
            .field("fallback_actions", &self.fallback_actions)
            .finish()
    }
}

fn pursue_goal(game: &CoreGame, goal: BotGoal) -> Option<Action> {
    let bot = game.active_character();
    println!("--------------------");
    println!("Run fighter behaviour ({} #{})", bot.name, bot.id());
    println!("--------------------");
    assert!(!bot.player_controlled());

    println!("bot AP: {}", bot.action_points.current());

    dbg!(("bot goal: {:?}", &goal));
    let mut path_to_goal;

    match goal.action {
        (BotAction::Attack, goal_target) => {
            let goal_target = goal_target.as_ref().unwrap();
            if bot.can_attack(bot.attack_action().unwrap())
                && attack_reaches(bot, goal_target, &game.pathfind_grid)
            {
                println!("bot attacks target");
                return Some(attack_action(bot, goal_target));
            }
            let weapon_range = bot.attack_weapon_range().unwrap().into_range();
            path_to_goal = find_path(game, bot, &goal_target, weapon_range);
        }
        (BotAction::SingleEnemyTarget(ability), goal_target) => {
            let goal_target = goal_target.as_ref().unwrap();
            if bot.can_use_ability(ability)
                && bot.reaches_with_ability(ability, &[], goal_target.pos())
            {
                println!("bot uses ability on player");
                return Some(simple_targetted_ability_action(ability, goal_target));
            } else {
                println!("-------");
                println!("Bot cannot use ability or doesn't reach target");
                dbg!(ability.target.range(&[]));
                dbg!(bot.pos());
                dbg!(goal_target.pos());
                dbg!(distance_between(bot.pos(), goal_target.pos()));
                dbg!(sq_distance_between(bot.pos(), goal_target.pos()));
                println!("-------");
            }
            let range = ability.target.range(&[]).unwrap();
            path_to_goal = find_path(game, bot, &goal_target, range);
        }
        (BotAction::NonTarget(ability), _) => {
            if bot.can_use_ability(ability) {
                return Some(Action::UseAbility {
                    ability,
                    enhancements: vec![],
                    target: ActionTarget::None,
                });
            }
            path_to_goal = None;
        }
        (BotAction::SingleFriendlyTarget(ability), goal_target) => {
            let goal_target = goal_target.as_ref().unwrap();
            if bot.can_use_ability(ability)
                && bot.reaches_with_ability(ability, &[], goal_target.pos())
            {
                println!("bot uses ability on some bot");
                return Some(simple_targetted_ability_action(ability, goal_target));
            }
            let range = ability.target.range(&[]).unwrap();
            path_to_goal = find_path(game, bot, &goal_target, range);
        }
    }

    if let Some(path) = path_to_goal {
        if path.total_distance <= bot.remaining_movement.get() {
            println!("BOT MOVING PATH: {:?}", path);
            return convert_path_to_move_action(bot, path);
        } else {
            println!(
                "Bot will not reach goal this turn; look for other things to do before moving"
            );
            // Restore it in case no fallback action gets taken; then we'll want to start moving
            // even though we cannot reach the goal.
            path_to_goal = Some(path);
        }
    } else {
        println!("bot's goal didn't involve movement");
    }

    let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();
    player_chars.shuffle();

    let mut bot_chars: Vec<&Rc<Character>> = game.enemies().collect();
    bot_chars.shuffle();

    for action in goal.fallback_actions {
        match action {
            BotAction::Attack => {
                if bot.can_attack(bot.attack_action().unwrap()) {
                    for player_char in &player_chars {
                        if attack_reaches(bot, player_char, &game.pathfind_grid) {
                            println!("bot attacks someone before moving to target");
                            return Some(attack_action(bot, player_char));
                        }
                    }
                }
            }
            BotAction::SingleEnemyTarget(ability) => {
                if bot.can_use_ability(ability) {
                    for player_char in &player_chars {
                        if may_use(bot, ability, Some(player_char))
                            && bot.reaches_with_ability(ability, &[], player_char.pos())
                        {
                            println!("bot uses ability on some player before moving to target");
                            return Some(simple_targetted_ability_action(ability, player_char));
                        }
                    }
                }
            }
            BotAction::NonTarget(ability) => {
                if may_use(bot, ability, None) && bot.can_use_ability(ability) {
                    println!("bot uses nontargeted ability before moving to target");
                    return Some(Action::UseAbility {
                        ability,
                        enhancements: vec![],
                        target: ActionTarget::None,
                    });
                }
            }
            BotAction::SingleFriendlyTarget(ability) => {
                if bot.can_use_ability(ability) {
                    for bot_char in &bot_chars {
                        if may_use(bot, ability, Some(bot_char))
                            && bot.reaches_with_ability(ability, &[], bot_char.pos())
                        {
                            println!("bot uses ability on some bot before moving to target");
                            return Some(simple_targetted_ability_action(ability, bot_char));
                        }
                    }
                }
            }
        }
    }

    if let Some(path) = path_to_goal {
        println!("No fallback action was taken. Let's move then.");
        println!("BOT MOVING PATH: {:?}", path);
        return convert_path_to_move_action(bot, path);
    }

    println!("No bot action");

    // There are valid cases where a bot cannot take any action (such as not having enough AP, and not enough movement to get anywhere interesting)
    None
}

fn candidate_actions(bot: &Character) -> Vec<BotAction> {
    let mut candidates = vec![BotAction::Attack];
    dbg!(bot.name);
    //for a in bot.usable_abilities() {
    for a in bot.known_abilities() {
        dbg!(a.name);
        //if may_use(bot, a) {
        let candidate = match a.target {
            AbilityTarget::Enemy { .. } => BotAction::SingleEnemyTarget(a),
            AbilityTarget::None { .. } => BotAction::NonTarget(a),
            AbilityTarget::Ally { .. } => BotAction::SingleFriendlyTarget(a),
            unhandled => todo!("{:?}", unhandled),
        };
        candidates.push(candidate);
        //}
    }
    CustomShuffle::shuffle(&mut candidates);
    candidates
}

fn find_path_to_attack_target(
    game: &CoreGame,
    bot: &Character,
    target: &&Rc<Character>,
) -> Option<Path> {
    let attack = bot.attack_action().unwrap();
    let weapon_range = bot.weapon(attack.hand).unwrap().range;

    find_path(game, bot, target, weapon_range.into_range())
}

fn find_path(
    game: &CoreGame,
    bot: &Character,
    target: &&Rc<Character>,
    action_range: Range,
) -> Option<Path> {
    let proximity_squared = match action_range {
        Range::Melee => CENTER_MELEE_RANGE_SQUARED,
        // Strictly speaking we may be requesting to get slightly closer to the target than necessary
        // since ability/weapon ranges are measured from the actor's center to any cell occupied by
        // the target (and the proximity value used by the pathfinder is measured from center to center),
        // but previously we converted from that range to a supposed 'center to center' range
        // in a way that was completely unsound and resulted in an overestimation of the action's range
        // in certain cases which meant: the bot moved to a location where it thought it would reach, but
        // then it didn't reach.
        Range::Ranged(r) => r.pow(2) as f32,
        Range::ExtendableRanged(r) => r.pow(2) as f32,
        Range::Float(r) => r.powf(2.0),
    };
    game.pathfind_grid.find_shortest_path_to_proximity(
        bot.id(),
        bot.pos(),
        target.pos(),
        proximity_squared,
        EXPLORATION_RANGE,
    )
}

#[derive(Copy, Clone, PartialEq)]
enum BotAction {
    Attack,
    SingleEnemyTarget(Ability),
    SingleFriendlyTarget(Ability),
    NonTarget(Ability),
}

impl std::fmt::Debug for BotAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Attack => write!(f, "Attack"),
            Self::SingleEnemyTarget(ability) => f
                .debug_tuple("SingleEnemyTarget")
                .field(&ability.name)
                .finish(),
            Self::SingleFriendlyTarget(ability) => f
                .debug_tuple("SingleFriendlyTarget")
                .field(&ability.name)
                .finish(),
            Self::NonTarget(ability) => f.debug_tuple("NonTarget").field(&ability.name).finish(),
        }
    }
}

fn attack_action(bot: &Character, target: &Character) -> Action {
    let mut enhancements = vec![];

    if random_bool(0.5) {
        if let Some(e) = bot.known_attack_enhancements.borrow().first() {
            enhancements.push(*e);
        }
    }

    Action::Attack {
        hand: HandType::MainHand,
        enhancements,
        target: target.id(),
    }
}

fn simple_targetted_ability_action(ability: Ability, target: &Character) -> Action {
    Action::UseAbility {
        ability,
        enhancements: vec![],
        target: ActionTarget::Character(target.id(), None),
    }
}

fn attack_reaches(bot: &Character, target: &Character, pathfind_grid: &PathfindGrid) -> bool {
    println!("bot::attack_reaches()...");
    let action_reach = bot
        .reaches_with_attack(HandType::MainHand, target.pos(), iter::empty())
        .1;
    if action_reach == ActionReach::No {
        return false;
    }

    !pathfind_grid.obstructed_line_of_sight(bot.pos(), target.pos())
}

fn run_normal_behaviour(game: &CoreGame) -> Option<Action> {
    let bot = game.active_character();
    assert!(!bot.player_controlled());

    let mut attack_range = None;

    let is_ranged_attacker = bot
        .weapon(HandType::MainHand)
        .map(|weapon| !weapon.is_melee())
        .unwrap_or(false);

    let mut player_chars: Vec<&Rc<Character>> = game.player_characters().collect();

    let bot_pos = bot.position.get();

    // Flee out of melee
    if is_ranged_attacker {
        if let Some(adj_player_char) = player_chars
            .iter()
            .find(|ch| are_entities_within_melee(bot_pos, ch.pos()))
        {
            let safe_adjacent_positions: Vec<Position> = adjacent_cells(bot_pos)
                .into_iter()
                .filter(|pos| {
                    game.pathfind_grid.is_free(Some(bot.id()), *pos)
                        && !player_chars
                            .iter()
                            .any(|ch| are_entities_within_melee(ch.pos(), *pos))
                })
                .collect();

            if let Some(safe_pos) = ChooseRandom::choose(&safe_adjacent_positions[..]) {
                if let Some(path) =
                    game.pathfind_grid
                        .find_shortest_path_to(bot.id(), bot_pos, *safe_pos)
                {
                    println!("Bot flees from {}: {:?}", adj_player_char.name, path);
                    return convert_path_to_move_action(bot, path);
                }
            }
        }
    }

    if let Some(attack) = bot.attack_action() {
        attack_range = Some(bot.weapon(attack.hand).unwrap().range);
        CustomShuffle::shuffle(&mut player_chars);
        for player_char in player_chars {
            if attack_reaches(bot, player_char, &game.pathfind_grid) {
                if bot.can_attack(attack) {
                    return Some(attack_action(bot, player_char));
                } else {
                    println!("bot reaches a player char but doesn't have enough AP to attack. Let it chill.");
                    return None;
                }
            }
        }
    }

    let mut shortest_path_to_some_player: Option<Path> = None;

    for player_pos in &game.player_positions() {
        let maybe_path = if let Some(range) = attack_range {
            game.pathfind_grid.find_shortest_path_to_proximity(
                bot.id(),
                bot_pos,
                *player_pos,
                range.center_to_center_squared(),
                EXPLORATION_RANGE,
            )
        } else {
            game.pathfind_grid.find_shortest_path_to_adjacent(
                bot.id(),
                bot_pos,
                *player_pos,
                EXPLORATION_RANGE,
            )
        };

        if let Some(path) = maybe_path {
            //dbg!(bot_pos, player_pos, &path);
            if let Some(shortest) = &shortest_path_to_some_player {
                if path.total_distance < shortest.total_distance {
                    shortest_path_to_some_player = Some(path);
                }
            } else {
                shortest_path_to_some_player = Some(path);
            }
        }
    }

    if let Some(path) = shortest_path_to_some_player {
        return convert_path_to_move_action(bot, path);
    }

    // If a character starts its turn with 0 AP, it can't take any actions, so None is a valid case here
    None
}

pub fn convert_path_to_move_action(character: &Character, path: Path) -> Option<Action> {
    let remaining_free_movement = character.remaining_movement.get();
    dbg!(remaining_free_movement);
    //let max_sprint_usage = character.stamina.current();
    let mut positions = vec![];
    let mut total_distance = 0.0;
    for (dist, pos) in path.positions.iter().copied() {
        if dist <= remaining_free_movement {
            positions.push(pos);
            total_distance = dist;
        }
    }

    let extra_cost = 0; // ((total_distance - remaining_free_movement).ceil() as u32).max(0);

    if total_distance > 0.0 {
        Some(Action::Move {
            total_distance,
            positions,
            extra_cost,
        })
    } else {
        println!("---");
        println!(
            "bot path resulting in no action: {:?}, from pos: {:?}",
            path,
            character.pos()
        );
        println!("---");
        None
    }
}

fn may_use(bot: &Character, ability: Ability, target: Option<&Character>) -> bool {
    match ability.id {
        AbilityId::Brace => !bot.conditions.borrow().has(&Condition::Protected),
        AbilityId::Inspire => !bot.conditions.borrow().has(&Condition::Inspired),
        AbilityId::MagiInflictHorrors => {
            !target.unwrap().conditions.borrow().has(&Condition::Slowed)
        }
        AbilityId::MagiInflictWounds => !target
            .unwrap()
            .conditions
            .borrow()
            .has(&Condition::Bleeding),
        _ => true,
    }
}

pub fn bot_choose_attack_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnAttackedReaction> {
    // TODO: it needs to be more intuitive/clear for player how/when/why bot reacts
    None
}

pub fn bot_choose_hit_reaction(
    game: &CoreGame,
    reactor_id: CharacterId,
    is_within_melee: bool,
) -> Option<OnHitReaction> {
    // TODO: it needs to be more intuitive/clear for player how/when/why bot reacts
    return None;

    let reactions = game
        .characters
        .get(reactor_id)
        .usable_on_hit_reactions(is_within_melee);
    if let Some((_, reaction)) = reactions.first() {
        Some(*reaction)
    } else {
        None
    }
}

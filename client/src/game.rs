//! The in-game state: talks to the server, moves the player, handles input
//! and draws the 3D world.

use std::collections::HashMap;

use shared::dungeon;
use shared::props::{Colliders, WAYSTONE_RANGE, WAYSTONE_SPOT};

use macroquad::prelude::*;
use shared::data::*;
use shared::net::Connection;
use shared::protocol::*;
use shared::world::*;

use crate::hud::{self, Layout};
use crate::render::{self, Batch, Look, Pose, Scene};

/// How far Tab looks for enemies.
const TAB_RANGE: f32 = 45.0;
const TURN_SPEED: f32 = 2.6;
const GRAVITY: f32 = 25.0;
const JUMP_SPEED: f32 = 8.0;
const MOUSE_SENSITIVITY: f32 = 0.006;

/// Another entity as the client knows it.
pub struct Ent {
    pub view: EntityView,
    /// Smoothed position and facing for drawing.
    pub pos: Vec3,
    pub yaw: f32,
    walk: f32,
    /// Seconds since the last attack started, for the swing animation.
    swing: f32,
    /// Attacks so far, to alternate moves.
    combo: u32,
    /// Seconds since last hurt.
    hurt: f32,
}

impl Ent {
    pub fn look(&self) -> Look {
        Look {
            kind: self.view.kind,
            appearance: self.view.appearance,
            gear: self.view.gear,
            seed: self.view.id,
        }
    }
}

pub struct FloatText {
    pub entity: EntityId,
    pub anchor: Vec3,
    pub text: String,
    pub color: Color,
    pub big: bool,
    pub age: f32,
}

pub struct ChatLine {
    pub text: String,
    pub color: Color,
}

/// A mouse button held down, and whether it has been dragged (camera control)
/// or is still a click.
#[derive(Clone, Copy)]
struct Press {
    start: Vec2,
    dragged: bool,
}

/// Which windows are open.
#[derive(Default)]
pub struct Windows {
    pub bags: bool,
    pub character: bool,
    /// The skills window (K), and which skill's tab is showing.
    pub skills: bool,
    pub skill: Skill,
    /// The merchant whose wares are shown.
    pub vendor: Option<EntityId>,
    pub talents: bool,
    pub sandbox: bool,
    /// The world map covers the screen.
    pub map: bool,
    /// The quest giver whose quests are shown.
    pub quest_giver: Option<EntityId>,
    pub quest_log: bool,
    /// Where the waystone you're at can take you.
    pub travel: bool,
}

impl Windows {
    pub fn any(&self) -> bool {
        self.bags
            || self.character
            || self.skills
            || self.vendor.is_some()
            || self.talents
            || self.sandbox
            || self.map
            || self.quest_giver.is_some()
            || self.quest_log
            || self.travel
    }
}

pub struct Game {
    conn: Connection,
    pub class: Class,
    pub my_id: Option<EntityId>,
    pub entities: HashMap<EntityId, Ent>,
    pub me: SelfView,
    pub target: Option<EntityId>,

    // Our own movement; the server trusts it within reason.
    pub pos: Vec3,
    pub yaw: f32,
    vel_y: f32,
    grounded: bool,
    moving: bool,
    send_timer: f32,
    last_sent: (Vec3, f32, bool),
    heartbeat: f32,

    pub cam_yaw: f32,
    cam_pitch: f32,
    cam_dist: f32,
    lmb: Option<Press>,
    rmb: Option<Press>,
    cursor_locked: bool,
    last_mouse: Vec2,

    pub chat: Vec<ChatLine>,
    pub chat_input: Option<String>,
    pub errors: Vec<(String, f32)>,
    /// Ground a boss has marked, about to be hit.
    hazards: Vec<HazardView>,
    pub floats: Vec<FloatText>,
    pub banner: Option<(String, String, f32)>,
    /// "Interrupted" and similar, shown on the cast bar.
    pub cast_flash: Option<(String, f32)>,
    /// Quest progress, shown in the middle of the screen for a moment.
    pub quest_flash: Option<(String, f32)>,
    vfx: crate::vfx::Vfx,
    pub windows: Windows,
    pub show_help: bool,
    pub menu_open: bool,
    pub time: f32,
    batch: Batch,
    /// The starting area you're in (or came from, in the Sunken Vault),
    /// and what you can bump into there.
    pub zone: Zone,
    /// Where you are: a starting area or a copy of the vault.
    pub place: Place,
    colliders: Colliders,
    /// Show the zone name once we know where we are.
    banner_due: bool,
    /// The world map picture of the current zone.
    pub map_texture: Option<(Zone, Texture2D)>,
    /// What the talents add up to (for run speed).
    bonuses: shared::talents::Bonuses,
}

pub enum Outcome {
    Continue,
    /// Back to the character list on the same connection.
    Logout,
    /// Back to the login screen, with a message.
    Disconnected(String),
}

impl Game {
    /// Starts playing on a connection whose character has just entered the world.
    pub fn new(conn: Connection, class: Class) -> Self {
        let (mx, my) = mouse_position();
        let mut game = Self {
            conn,
            class,
            my_id: None,
            entities: HashMap::new(),
            me: SelfView::default(),
            target: None,
            pos: Zone::Amberfall.graveyard(),
            yaw: 0.0,
            vel_y: 0.0,
            grounded: true,
            moving: false,
            send_timer: 0.0,
            last_sent: (Vec3::ZERO, 0.0, false),
            heartbeat: 0.0,
            cam_yaw: 0.0,
            cam_pitch: 0.32,
            cam_dist: 9.0,
            lmb: None,
            rmb: None,
            cursor_locked: false,
            last_mouse: vec2(mx, my),
            chat: Vec::new(),
            chat_input: None,
            errors: Vec::new(),
            hazards: Vec::new(),
            floats: Vec::new(),
            banner: None,
            cast_flash: None,
            quest_flash: None,
            vfx: Default::default(),
            windows: Windows::default(),
            show_help: true,
            menu_open: false,
            time: 0.0,
            batch: Batch::new(),
            zone: Zone::Amberfall,
            place: Place::Zone(Zone::Amberfall),
            colliders: Colliders::for_zone(Zone::Amberfall),
            banner_due: true,
            map_texture: None,
            bonuses: Default::default(),
        };
        game.system("Welcome! Press H to show or hide the controls.");
        game
    }

    /// Gives the connection back after logging out.
    pub fn into_connection(mut self) -> Connection {
        self.lock_cursor(false);
        self.conn
    }

    pub fn my_view(&self) -> Option<&EntityView> {
        self.my_id
            .and_then(|id| self.entities.get(&id))
            .map(|e| &e.view)
    }

    pub fn target_ent(&self) -> Option<&Ent> {
        self.target.and_then(|t| self.entities.get(&t))
    }

    pub fn level(&self) -> u8 {
        self.my_view().map_or(1, |v| v.level)
    }

    fn send(&mut self, msg: ClientMsg) {
        // A failed send shows up as a disconnect on the next poll.
        let _ = self.conn.send(&msg);
    }

    fn system(&mut self, text: &str) {
        self.chat.push(ChatLine {
            text: text.to_string(),
            color: Color::new(1.0, 0.9, 0.3, 1.0),
        });
    }

    pub fn error(&mut self, text: &str) {
        self.errors.retain(|(t, _)| t != text);
        self.errors.push((text.to_string(), 0.0));
        if self.errors.len() > 3 {
            self.errors.remove(0);
        }
    }

    fn lock_cursor(&mut self, lock: bool) {
        if self.cursor_locked != lock {
            self.cursor_locked = lock;
            set_cursor_grab(lock);
            show_mouse(!lock);
        }
    }

    pub fn frame(&mut self, scene: &Scene) -> Outcome {
        let dt = get_frame_time().min(0.1);
        self.time += dt;
        if let Err(reason) = self.network() {
            self.lock_cursor(false);
            return Outcome::Disconnected(reason);
        }
        let mut layout = Layout::new(&self.windows);
        let mut draggable = layout.draggable();
        if self.windows.quest_giver.is_some() {
            let level = self.level();
            let giver =
                crate::quests_ui::giver_layout(self.zone, &self.me.quests, level, &self.me.bags)
                    .window;
            layout.quest_window = Some(giver);
            draggable.push((crate::drag::Win::QuestGiver, giver));
        }
        if self.windows.quest_log {
            let l = crate::quests_ui::log_layout(&self.me.quests);
            layout.quest_window = Some(match layout.quest_window {
                Some(r) => r.combine_with(l.window),
                None => l.window,
            });
            draggable.push((crate::drag::Win::QuestLog, l.window));
        }
        layout.party = hud::PartyLayout::new(self, layout.target_frame);
        if let Some(outcome) = self.input(&layout, &draggable) {
            self.lock_cursor(false);
            return outcome;
        }
        self.simulate(dt);
        self.send_movement(dt);

        self.update_zone();
        // The map picture also backs the minimap, so make it once we're in.
        if self.my_id.is_some()
            && !self.in_dungeon()
            && self
                .map_texture
                .as_ref()
                .is_none_or(|(z, _)| *z != self.zone)
        {
            self.map_texture = Some((self.zone, crate::panels::map_texture(self.zone)));
        }
        let cam = self.camera();
        if self.in_dungeon() {
            clear_background(render::dungeon_theme().fog);
        } else {
            render::draw_sky(&cam, self.zone, self.time, |p| hud::project(&cam, p));
        }
        set_camera(&cam);
        if self.in_dungeon() {
            scene.begin_dungeon();
        } else {
            scene.begin_3d(self.zone);
        }
        self.vfx.begin(cam.position);
        self.draw_world(scene);
        scene.end_3d();
        set_default_camera();
        hud::draw(self, &layout, &cam);
        Outcome::Continue
    }

    /// Notices when you arrive in a different starting area.
    fn update_zone(&mut self) {
        if self.my_id.is_none_or(|id| !self.entities.contains_key(&id)) {
            return;
        }
        let place = Place::at(self.pos);
        let arrived = place != self.place;
        self.place = place;
        match place {
            Place::Dungeon(_) => {
                if arrived || self.banner_due {
                    self.banner_due = false;
                    self.banner = Some((dungeon::NAME.into(), "Dungeon".into(), 0.0));
                }
            }
            Place::Zone(zone) => {
                if zone != self.zone {
                    self.zone = zone;
                    self.colliders = Colliders::for_zone(zone);
                }
                if arrived || self.banner_due {
                    self.banner_due = false;
                    self.banner = Some((zone.name().into(), zone.subtitle(), 0.0));
                }
            }
        }
        // Walking away from a waystone closes its window.
        if self.windows.travel && !self.near_waystone() {
            self.windows.travel = false;
        }
    }

    pub fn in_dungeon(&self) -> bool {
        matches!(self.place, Place::Dungeon(_))
    }

    /// The waystone where you are: in your town, or by the vault's entrance.
    pub fn waystone(&self) -> Vec3 {
        match self.place {
            Place::Zone(z) => z.ground_local(WAYSTONE_SPOT),
            Place::Dungeon(i) => dungeon::to_world(i, dungeon::EXIT_STONE),
        }
    }

    pub fn near_waystone(&self) -> bool {
        flat_distance(self.pos, self.waystone()) <= WAYSTONE_RANGE
    }

    /// Asks to go somewhere from the waystone you're at.
    pub fn travel(&mut self, to: Destination) {
        self.windows.travel = false;
        self.send(ClientMsg::Travel(to));
    }

    // ---- Network ----

    fn network(&mut self) -> Result<(), String> {
        loop {
            match self.conn.poll::<ServerMsg>() {
                Ok(Some(msg)) => self.handle(msg)?,
                Ok(None) => break,
                Err(e) => return Err(format!("Lost connection to the server ({e}).")),
            }
        }
        self.conn
            .flush()
            .map_err(|e| format!("Lost connection to the server ({e})."))
    }

    fn handle(&mut self, msg: ServerMsg) -> Result<(), String> {
        match msg {
            ServerMsg::Welcome { id } => self.my_id = Some(id),
            ServerMsg::Rejected(reason) => return Err(reason),
            ServerMsg::SetPosition { pos, yaw } => {
                // Only swing the camera around after a long jump (a respawn),
                // not for a charge or a corrected step.
                if pos.distance(self.pos) > 40.0 {
                    self.cam_yaw = yaw;
                }
                self.pos = pos;
                self.yaw = yaw;
                self.vel_y = 0.0;
            }
            ServerMsg::Snapshot(mut snap) => {
                let first = self
                    .my_id
                    .is_some_and(|id| !self.entities.contains_key(&id));
                let mut seen = std::collections::HashSet::new();
                for view in snap.entities {
                    seen.insert(view.id);
                    match self.entities.get_mut(&view.id) {
                        Some(e) => e.view = view,
                        None => {
                            let (pos, yaw) = (view.pos, view.yaw);
                            self.entities.insert(
                                view.id,
                                Ent {
                                    view,
                                    pos,
                                    yaw,
                                    walk: 0.0,
                                    swing: 10.0,
                                    combo: 0,
                                    hurt: 10.0,
                                },
                            );
                        }
                    }
                }
                self.entities.retain(|id, _| seen.contains(id));
                self.hazards = std::mem::take(&mut snap.hazards);
                if snap.me.talents != self.me.talents {
                    self.bonuses = shared::talents::Bonuses::new(self.class, &snap.me.talents);
                }
                self.me = snap.me;
                if !self.me.sandbox {
                    self.windows.sandbox = false;
                }
                // Our saved position arrives with the first snapshot.
                if first && let Some(me) = self.my_id.and_then(|id| self.entities.get(&id)) {
                    self.pos = me.view.pos;
                    self.yaw = me.view.yaw;
                    self.cam_yaw = me.view.yaw;
                    self.last_sent = (self.pos, self.yaw, false);
                }
                if self.target.is_some_and(|t| !self.entities.contains_key(&t)) {
                    self.target = None;
                }
            }
            ServerMsg::Event(event) => self.event(event),
            // Only meaningful on the character screen.
            ServerMsg::Characters(_) | ServerMsg::CharacterError(_) => {}
        }
        Ok(())
    }

    /// Where an entity is, for spell effects.
    fn anchor(&self, id: EntityId) -> Option<crate::vfx::Anchor> {
        self.entities.get(&id).map(|e| crate::vfx::Anchor {
            pos: e.pos,
            height: if e.view.dead {
                0.6
            } else {
                render::model_height(e.view.kind, e.view.appearance)
            },
            yaw: e.yaw,
        })
    }

    fn name_of(&self, id: EntityId) -> String {
        if Some(id) == self.my_id {
            return "You".into();
        }
        self.entities
            .get(&id)
            .map_or_else(|| "Something".into(), |e| e.view.name.clone())
    }

    fn float(&mut self, entity: EntityId, text: String, color: Color, big: bool) {
        let anchor = self.entities.get(&entity).map_or(self.pos, |e| e.pos);
        self.floats.push(FloatText {
            entity,
            anchor,
            text,
            color,
            big,
            age: 0.0,
        });
    }

    fn event(&mut self, event: GameEvent) {
        let me = self.my_id;
        match event {
            GameEvent::Damage {
                source,
                target,
                amount,
                absorbed,
                crit,
                ability,
            } => {
                if ability.is_none()
                    && let Some(e) = self.entities.get_mut(&source)
                {
                    e.swing = 0.0;
                    e.combo = e.combo.wrapping_add(1);
                }
                if amount > 0
                    && let Some(e) = self.entities.get_mut(&target)
                {
                    e.hurt = 0.0;
                }
                self.vfx.hit(target, ability, crit);
                if Some(source) == me || Some(target) == me {
                    let color = if Some(target) == me {
                        Color::new(1.0, 0.3, 0.25, 1.0)
                    } else if crit {
                        Color::new(1.0, 0.85, 0.2, 1.0)
                    } else {
                        WHITE
                    };
                    let mut text = if amount > 0 || absorbed == 0 {
                        amount.to_string()
                    } else {
                        String::new()
                    };
                    if crit {
                        text.push('!');
                    }
                    if absorbed > 0 {
                        text = format!("{text} ({absorbed} absorbed)").trim().to_string();
                    }
                    self.float(target, text, color, crit);
                }
            }
            GameEvent::Heal {
                source,
                target,
                amount,
                crit,
                ..
            } => {
                if (Some(source) == me || Some(target) == me) && amount > 0 {
                    let text = if crit {
                        format!("+{amount}!")
                    } else {
                        format!("+{amount}")
                    };
                    self.float(target, text, Color::new(0.3, 1.0, 0.35, 1.0), crit);
                }
            }
            GameEvent::AbilityUsed {
                caster,
                target,
                ability: id,
            } => {
                let a = ability(id);
                let class = match self.entities.get(&caster).map(|e| e.view.kind) {
                    Some(EntityKind::Player(c)) => Some(c),
                    _ => None,
                };
                let anchors: Vec<(EntityId, crate::vfx::Anchor)> = [Some(caster), target]
                    .into_iter()
                    .flatten()
                    .filter_map(|id| self.anchor(id).map(|a| (id, a)))
                    .collect();
                self.vfx.ability_used(id, caster, target, class, |id| {
                    anchors.iter().find(|(e, _)| *e == id).map(|(_, a)| *a)
                });
                if a.range <= MELEE_RANGE
                    && !a.projectile
                    && a.targeting.needs_enemy()
                    && let Some(e) = self.entities.get_mut(&caster)
                {
                    e.swing = 0.0;
                    e.combo = e.combo.wrapping_add(1);
                }
            }
            GameEvent::Interrupted { target, .. } => {
                if Some(target) == me {
                    self.cast_flash = Some(("Interrupted".into(), 0.0));
                }
                self.float(
                    target,
                    "Interrupted".into(),
                    Color::new(0.9, 0.9, 0.9, 1.0),
                    false,
                );
            }
            GameEvent::Evade { target } => {
                self.float(
                    target,
                    "Evade".into(),
                    Color::new(0.85, 0.85, 0.85, 1.0),
                    false,
                );
            }
            GameEvent::Died { id, killer } => {
                if Some(id) == me {
                    self.chat.push(ChatLine {
                        text: format!(
                            "You were killed by {}.",
                            killer.map_or("something".into(), |k| self.name_of(k))
                        ),
                        color: Color::new(1.0, 0.4, 0.4, 1.0),
                    });
                    self.cast_flash = None;
                }
            }
            GameEvent::LevelUp { id, level } => {
                self.vfx.level_up(id);
                if Some(id) == me {
                    self.banner = Some((format!("Level {level}!"), String::new(), 0.0));
                    self.system(&format!("Congratulations, you have reached level {level}!"));
                }
            }
            GameEvent::Learned(id) => {
                let name = ability(id).name;
                self.system(&format!("You have learned a new ability: {name}."));
                if let Some((_, sub, _)) = &mut self.banner {
                    *sub = format!("New ability: {name}");
                }
            }
            GameEvent::Xp { amount, from } => {
                self.chat.push(ChatLine {
                    text: format!("{from} dies, you gain {amount} experience."),
                    color: Color::new(0.75, 0.6, 1.0, 1.0),
                });
            }
            GameEvent::Looted { money, items } => {
                if money > 0 {
                    self.chat.push(ChatLine {
                        text: format!("You loot {}.", format_money(money)),
                        color: Color::new(1.0, 0.85, 0.35, 1.0),
                    });
                }
                for (id, n) in items {
                    let it = item(id);
                    let text = if n > 1 {
                        format!("You receive loot: {} x{n}.", it.name)
                    } else {
                        format!("You receive loot: {}.", it.name)
                    };
                    self.chat.push(ChatLine {
                        text,
                        color: hud::quality_color(it.quality),
                    });
                }
            }
            GameEvent::Crafted(id) => {
                let it = item(id);
                self.chat.push(ChatLine {
                    text: format!("You create {}.", it.name),
                    color: hud::quality_color(it.quality),
                });
            }
            GameEvent::Bought { item: id, price } => {
                let it = item(id);
                self.chat.push(ChatLine {
                    text: format!("You buy {} for {}.", it.name, format_money(price)),
                    color: hud::quality_color(it.quality),
                });
            }
            GameEvent::Sold {
                item: id,
                count,
                money,
            } => {
                let it = item(id);
                let what = if count > 1 {
                    format!("{} x{count}", it.name)
                } else {
                    it.name.to_string()
                };
                self.chat.push(ChatLine {
                    text: format!("You sell {what} for {}.", format_money(money)),
                    color: Color::new(1.0, 0.85, 0.35, 1.0),
                });
            }
            GameEvent::QuestAccepted(id) => {
                let q = shared::quests::quest(id);
                self.system(&format!("Quest accepted: {}", q.name));
            }
            GameEvent::QuestProgress {
                quest: id,
                progress,
            } => {
                let q = shared::quests::quest(id);
                let s = crate::quests_ui::Status::InProgress(progress, q.needed());
                let line = crate::quests_ui::objective(q, s);
                let done = progress >= q.needed();
                self.chat.push(ChatLine {
                    text: line.clone(),
                    color: Color::new(1.0, 0.85, 0.3, 1.0),
                });
                self.quest_flash = Some((
                    if done {
                        format!("{} (Complete)", line)
                    } else {
                        line
                    },
                    0.0,
                ));
            }
            GameEvent::QuestComplete {
                quest: id,
                money,
                reward,
                ..
            } => {
                let q = shared::quests::quest(id);
                self.banner = Some((q.name.to_string(), "Quest complete!".into(), 0.0));
                self.system(&format!("{} completed.", q.name));
                self.chat.push(ChatLine {
                    text: format!("You receive {}.", format_money(money)),
                    color: Color::new(1.0, 0.85, 0.35, 1.0),
                });
                if let Some(r) = reward {
                    let it = item(r);
                    self.chat.push(ChatLine {
                        text: format!("You receive item: {}.", it.name),
                        color: hud::quality_color(it.quality),
                    });
                }
                if let Some(me) = self.my_id {
                    self.vfx.level_up(me);
                }
            }
            GameEvent::Error(text) => self.error(&text),
            GameEvent::Chat { from, text } => self.chat.push(ChatLine {
                text: format!("[{from}]: {text}"),
                color: WHITE,
            }),
            GameEvent::PartyChat { from, text } => self.chat.push(ChatLine {
                text: format!("[Party] [{from}]: {text}"),
                color: Color::new(0.55, 0.75, 1.0, 1.0),
            }),
            GameEvent::System(text) => self.system(&text),
        }
        if self.chat.len() > 100 {
            self.chat.drain(..self.chat.len() - 100);
        }
    }

    // ---- Input ----

    fn set_target(&mut self, target: Option<EntityId>) {
        if self.target != target {
            self.target = target;
            self.send(ClientMsg::SetTarget(target));
        }
    }

    pub fn is_hostile(&self, e: &EntityView) -> bool {
        e.kind.hostile_to(EntityKind::Player(self.class))
    }

    /// Standing still and attacking? Turn to face the target first.
    fn face_target(&mut self) {
        if self.moving {
            return;
        }
        let Some(t) = self.target_ent() else { return };
        if !self.is_hostile(&t.view) || t.view.dead || t.pos.distance(self.pos) < 0.3 {
            return;
        }
        self.yaw = yaw_towards(self.pos, t.pos);
        self.last_sent = (self.pos, self.yaw, self.moving);
        self.send(ClientMsg::Move {
            pos: self.pos,
            yaw: self.yaw,
            moving: false,
        });
    }

    pub fn use_slot(&mut self, slot: usize) {
        let ability = self.class.abilities()[slot];
        if self.my_view().is_some_and(|v| v.dead) {
            self.error("You are dead.");
            return;
        }
        if UNLOCK_LEVELS[slot] > self.level() {
            self.error(&format!(
                "You learn {} at level {}.",
                shared::data::ability(ability).name,
                UNLOCK_LEVELS[slot]
            ));
            return;
        }
        if shared::data::ability(ability).targeting.needs_enemy() {
            self.face_target();
        }
        self.send(ClientMsg::UseAbility {
            ability,
            target: self.target,
        });
    }

    fn tab_target(&mut self, cam: &Camera3D) {
        let mut candidates: Vec<(f32, EntityId)> = self
            .entities
            .values()
            .filter(|e| self.is_hostile(&e.view) && !e.view.dead)
            .filter(|e| e.pos.distance(self.pos) <= TAB_RANGE)
            .filter(|e| {
                hud::project(cam, e.pos + Vec3::Y).is_some_and(|p| {
                    p.x >= 0.0 && p.y >= 0.0 && p.x <= screen_width() && p.y <= screen_height()
                })
            })
            .map(|e| (e.pos.distance(self.pos), e.view.id))
            .collect();
        if candidates.is_empty() {
            self.error("No enemies in sight.");
            return;
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let next = match self
            .target
            .and_then(|t| candidates.iter().position(|c| c.1 == t))
        {
            Some(i) => (i + 1) % candidates.len(),
            None => 0,
        };
        self.set_target(Some(candidates[next].1));
    }

    /// The entity under the mouse cursor, nearest to the camera first.
    fn pick(&self, cam: &Camera3D, mouse: Vec2) -> Option<EntityId> {
        self.entities
            .values()
            .filter(|e| Some(e.view.id) != self.my_id)
            .filter_map(|e| {
                let h = if e.view.dead {
                    0.8
                } else {
                    render::model_height(e.view.kind, e.view.appearance)
                };
                let r = render::model_radius(e.view.kind);
                let bottom = hud::project(cam, e.pos)?;
                let top = hud::project(cam, e.pos + Vec3::Y * h)?;
                let height = (bottom.y - top.y).max(8.0);
                let half_width = (height * r.max(h * 0.6) / h).max(14.0);
                let inside = mouse.y >= top.y - 6.0
                    && mouse.y <= bottom.y + 6.0
                    && (mouse.x - bottom.x).abs() <= half_width;
                inside.then(|| (e.pos.distance(cam.position), e.view.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    fn input(
        &mut self,
        layout: &Layout,
        draggable: &[(crate::drag::Win, Rect)],
    ) -> Option<Outcome> {
        let (mx, my) = mouse_position();
        let mouse = vec2(mx, my);
        let delta = mouse - self.last_mouse;
        self.last_mouse = mouse;
        let cam = self.camera();
        let dead = self.my_view().is_some_and(|v| v.dead);

        // Typing in chat. The key that sends or cancels a message doesn't
        // also count as a game key this frame.
        let typing = self.chat_input.is_some();
        if let Some(text) = &mut self.chat_input {
            while let Some(c) = get_char_pressed() {
                if !c.is_control() && text.chars().count() < 200 {
                    text.push(c);
                }
            }
            if is_key_pressed(KeyCode::Backspace) {
                text.pop();
            }
            if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
                let text = self.chat_input.take().unwrap();
                if !text.trim().is_empty() {
                    self.send(ClientMsg::Chat(text));
                }
            } else if is_key_pressed(KeyCode::Escape) {
                self.chat_input = None;
            }
        } else {
            while get_char_pressed().is_some() {}
        }

        if !typing && is_key_pressed(KeyCode::Escape) {
            if self.menu_open {
                self.menu_open = false;
            } else if self.windows.map {
                self.windows.map = false;
            } else if self.windows.any() {
                self.windows = Windows::default();
            } else if self.target.is_some() {
                self.set_target(None);
            } else {
                self.menu_open = true;
            }
        }

        let left_pressed = is_mouse_button_pressed(MouseButton::Left);
        let right_pressed = is_mouse_button_pressed(MouseButton::Right);
        if self.menu_open {
            self.lmb = None;
            self.rmb = None;
            self.lock_cursor(false);
            if left_pressed {
                if layout.menu_buttons[0].contains(mouse) {
                    self.menu_open = false;
                } else if layout.menu_buttons[1].contains(mouse) {
                    self.send(ClientMsg::Logout);
                    return Some(Outcome::Logout);
                } else if layout.menu_buttons[2].contains(mouse) {
                    std::process::exit(0);
                }
            }
            return None;
        }

        // Clicks on the interface. Grabbing a window's title bar drags it
        // instead.
        let over_ui = !self.cursor_locked && layout.blocks(mouse, self.target.is_some(), dead);
        let grabbed = left_pressed
            && over_ui
            && draggable
                .iter()
                .rev()
                .find(|(_, r)| r.contains(mouse))
                .is_some_and(|(win, r)| {
                    let on_bar = crate::drag::title_bar(*r).contains(mouse);
                    if on_bar {
                        crate::drag::grab(*win, mouse);
                    }
                    on_bar
                });
        crate::drag::update(mouse, is_mouse_button_down(MouseButton::Left));
        if (left_pressed || right_pressed) && over_ui && !grabbed {
            self.click_ui(layout, mouse, left_pressed);
        }

        // Mouse buttons in the world: drag to look around (the cursor locks
        // while dragging), click to target.
        if left_pressed && !over_ui {
            self.lmb = Some(Press {
                start: mouse,
                dragged: false,
            });
        }
        if right_pressed && !over_ui {
            self.rmb = Some(Press {
                start: mouse,
                dragged: false,
            });
        }
        for press in [&mut self.lmb, &mut self.rmb].into_iter().flatten() {
            if press.start.distance(mouse) > 4.0 {
                press.dragged = true;
            }
        }
        let dragging = self.lmb.is_some_and(|p| p.dragged) || self.rmb.is_some_and(|p| p.dragged);
        // Holding the right button always locks the cursor, even before it moves.
        self.lock_cursor(dragging || self.rmb.is_some());
        if dragging {
            self.cam_yaw -= delta.x * MOUSE_SENSITIVITY;
            self.cam_pitch = (self.cam_pitch + delta.y * MOUSE_SENSITIVITY).clamp(-0.4, 1.35);
        }
        if is_mouse_button_released(MouseButton::Left)
            && let Some(press) = self.lmb.take()
            && !press.dragged
            && let Some(id) = self.pick(&cam, press.start)
        {
            self.set_target(Some(id));
        }
        if is_mouse_button_released(MouseButton::Right)
            && let Some(press) = self.rmb.take()
            && !press.dragged
        {
            self.right_click(&cam, press.start);
        }
        let wheel = mouse_wheel().1;
        if wheel != 0.0 && !over_ui {
            self.cam_dist = (self.cam_dist - wheel.signum() * 1.2).clamp(2.5, 30.0);
        }

        if typing {
            return None;
        }
        for (slot, key) in [
            KeyCode::Key1,
            KeyCode::Key2,
            KeyCode::Key3,
            KeyCode::Key4,
            KeyCode::Key5,
            KeyCode::Key6,
            KeyCode::E,
        ]
        .into_iter()
        .enumerate()
        {
            if is_key_pressed(key) {
                self.use_slot(slot);
            }
        }
        if is_key_pressed(KeyCode::Tab) {
            self.tab_target(&cam);
        }
        if is_key_pressed(KeyCode::F1) {
            self.set_target(self.my_id);
        }
        if is_key_pressed(KeyCode::Q) {
            self.error("Q no longer strafes: use A and D. E is your seventh ability.");
        }
        if is_key_pressed(KeyCode::T) {
            if self.me.auto_attacking {
                self.send(ClientMsg::StopAttack);
            } else {
                self.face_target();
                self.send(ClientMsg::StartAttack);
            }
        }
        if is_key_pressed(KeyCode::H) {
            self.show_help = !self.show_help;
        }
        if is_key_pressed(KeyCode::B) {
            self.windows.bags = !self.windows.bags;
        }
        if is_key_pressed(KeyCode::C) {
            self.windows.character = !self.windows.character;
        }
        if is_key_pressed(KeyCode::K) {
            self.windows.skills = !self.windows.skills;
        }
        if is_key_pressed(KeyCode::M) {
            self.windows.map = !self.windows.map;
        }
        if is_key_pressed(KeyCode::F) {
            if self.near_waystone() {
                self.windows.travel = !self.windows.travel;
            } else {
                self.error("There's no waystone nearby.");
            }
        }
        if is_key_pressed(KeyCode::L) {
            self.windows.quest_log = !self.windows.quest_log;
        }
        if is_key_pressed(KeyCode::N) {
            self.windows.talents = !self.windows.talents;
        }
        if is_key_pressed(KeyCode::P) {
            if self.me.sandbox {
                self.windows.sandbox = !self.windows.sandbox;
            } else {
                self.error("The sandbox panel only works in sandbox mode.");
            }
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            self.chat_input = Some(String::new());
        } else if is_key_pressed(KeyCode::Slash) {
            self.chat_input = Some("/".into());
        }
        None
    }

    /// Right click in the world: loot a corpse, or target and attack.
    fn right_click(&mut self, cam: &Camera3D, at: Vec2) {
        let Some(id) = self.pick(cam, at) else { return };
        self.set_target(Some(id));
        let Some(e) = self.entities.get(&id) else {
            return;
        };
        if e.view.lootable {
            self.send(ClientMsg::Loot(id));
        } else if matches!(e.view.kind, EntityKind::Merchant(_)) {
            if e.pos.distance(self.pos) > MERCHANT_RANGE {
                self.error("You are too far away.");
            } else {
                self.windows.vendor = Some(id);
                self.windows.bags = true;
            }
        } else if matches!(e.view.kind, EntityKind::QuestGiver(_)) {
            if e.pos.distance(self.pos) > MERCHANT_RANGE {
                self.error("You are too far away.");
            } else {
                self.windows.quest_giver = Some(id);
            }
        } else if self.is_hostile(&e.view) && !e.view.dead {
            self.face_target();
            self.send(ClientMsg::StartAttack);
        }
    }

    fn quest_action(&mut self, action: crate::quests_ui::Action, giver: EntityId) {
        use crate::quests_ui::Action;
        match action {
            Action::Accept(quest) => self.send(ClientMsg::AcceptQuest { giver, quest }),
            Action::TurnIn(quest) => self.send(ClientMsg::TurnInQuest { giver, quest }),
            Action::Abandon(quest) => self.send(ClientMsg::AbandonQuest(quest)),
        }
    }

    fn click_ui(&mut self, layout: &Layout, mouse: Vec2, left: bool) {
        if self.windows.map {
            return;
        }
        if left && self.party_click(layout, mouse) {
            return;
        }
        let level = self.level();
        if let Some(giver) = self.windows.quest_giver
            && left
        {
            let l =
                crate::quests_ui::giver_layout(self.zone, &self.me.quests, level, &self.me.bags);
            if l.close.contains(mouse) {
                self.windows.quest_giver = None;
                return;
            }
            if let Some((_, action)) = l.buttons.iter().find(|(r, _)| r.contains(mouse)) {
                self.quest_action(*action, giver);
                return;
            }
            if l.window.contains(mouse) {
                return;
            }
        }
        if self.windows.quest_log && left {
            let l = crate::quests_ui::log_layout(&self.me.quests);
            if let Some((_, action)) = l.buttons.iter().find(|(r, _)| r.contains(mouse)) {
                self.quest_action(*action, 0);
                return;
            }
            if l.window.contains(mouse) {
                return;
            }
        }
        if self.windows.travel && left {
            let l = crate::panels::travel_layout(self.place, level);
            if l.close.contains(mouse) {
                self.windows.travel = false;
                return;
            }
            if let Some((_, to, _)) = l.buttons.iter().find(|(r, _, ok)| *ok && r.contains(mouse)) {
                self.travel(*to);
                return;
            }
            if l.window.contains(mouse) {
                return;
            }
        }
        if self.windows.sandbox
            && left
            && let Some(cmd) = crate::panels::sandbox_click(self.zone, level, mouse)
        {
            self.send(ClientMsg::Sandbox(cmd));
            return;
        }
        if self.windows.talents && layout.talents.is_some_and(|r| r.contains(mouse)) {
            match crate::panels::talent_click(mouse) {
                Some(i) if i == shared::talents::TALENTS => self.send(ClientMsg::ResetTalents),
                Some(i) => self.send(ClientMsg::LearnTalent(i)),
                None => {}
            }
            return;
        }
        if let Some(slot) = layout.hotbar.iter().position(|r| r.contains(mouse)) {
            self.use_slot(slot);
        } else if left && layout.player_frame.contains(mouse) {
            self.set_target(self.my_id);
        } else if left
            && self.my_view().is_some_and(|v| v.dead)
            && layout.release_button.contains(mouse)
        {
            self.send(ClientMsg::ReleaseSpirit);
        } else if let Some(i) = layout
            .bag_slots
            .iter()
            .position(|r| self.windows.bags && r.contains(mouse))
        {
            // With a merchant open, right click sells. Otherwise clicking
            // armor wears it and right clicking a potion drinks it.
            if let Some(Some((id, _))) = self.me.bags.get(i) {
                let kind = item(*id).kind;
                if let (Some(merchant), false) = (self.windows.vendor, left) {
                    self.send(ClientMsg::Sell { merchant, slot: i });
                } else if matches!(kind, ItemKind::Armor { .. } | ItemKind::Weapon { .. }) {
                    self.send(ClientMsg::Equip(i));
                } else if matches!(kind, ItemKind::Potion { .. } | ItemKind::Food { .. }) {
                    self.send(ClientMsg::UseItem(i));
                } else if !left {
                    self.error("You can't wear that. Use it in your skills (K).");
                }
            }
        } else if let Some(i) = layout
            .gear_slots
            .iter()
            .position(|r| self.windows.character && r.contains(mouse))
        {
            if self
                .my_view()
                .is_some_and(|v| hud::gear_row(v, i).0.is_some())
            {
                self.send(match Slot::ALL.get(i) {
                    Some(&slot) => ClientMsg::Unequip(slot),
                    None => ClientMsg::UnequipWeapon,
                });
            }
        } else if let Some(i) = layout
            .skill_tabs
            .iter()
            .position(|r| self.windows.skills && r.contains(mouse))
        {
            self.windows.skill = Skill::ALL[i];
        } else if let Some(i) = layout
            .craft_buttons
            .iter()
            .position(|r| self.windows.skills && r.contains(mouse))
            && left
            && let Some((recipe, _)) = self.windows.skill.recipes().nth(i)
        {
            self.send(ClientMsg::Craft(recipe));
        } else if let Some(merchant) = self.windows.vendor
            && let Some(i) = layout.vendor_buttons.iter().position(|r| r.contains(mouse))
        {
            self.send(ClientMsg::Buy {
                merchant,
                item: MERCHANT_GOODS[i],
            });
        }
    }

    /// Clicks on the party frames and buttons. True if one was hit.
    fn party_click(&mut self, layout: &Layout, mouse: Vec2) -> bool {
        let party = &layout.party;
        if let Some((_, accept, decline)) = party.popup {
            if accept.contains(mouse) {
                self.send(ClientMsg::PartyAccept);
                return true;
            }
            if decline.contains(mouse) {
                self.send(ClientMsg::PartyDecline);
                return true;
            }
        }
        // Open windows cover the party frames.
        if layout.over_window(mouse) {
            return false;
        }
        if let Some((_, name)) = party.kicks.iter().find(|(r, _)| r.contains(mouse)) {
            self.send(ClientMsg::PartyKick(name.clone()));
        } else if let Some((_, id)) = party.frames.iter().find(|(r, _)| r.contains(mouse)) {
            self.set_target(Some(*id));
        } else if party.leave.is_some_and(|r| r.contains(mouse)) {
            self.send(ClientMsg::PartyLeave);
        } else if let Some((_, name)) = party
            .invite_target
            .as_ref()
            .filter(|(r, _)| r.contains(mouse))
        {
            self.send(ClientMsg::PartyInvite(name.clone()));
        } else {
            return false;
        }
        true
    }

    // ---- Simulation ----

    /// 0 when stunned or rooted, less than 1 when slowed.
    fn movement_factor(&self) -> f32 {
        let Some(view) = self.my_view() else {
            return 0.0;
        };
        if view.dead {
            return 0.0;
        }
        let mut factor: f32 = 1.0;
        for aura in &view.auras {
            for effect in ability(aura.ability).effects {
                if let Effect::Aura { kind, .. } = effect {
                    match kind {
                        AuraKind::Stun | AuraKind::Root => factor = 0.0,
                        AuraKind::Slow(f) => factor *= *f,
                        AuraKind::Speed(f) => factor *= *f,
                        _ => {}
                    }
                }
            }
        }
        factor * (1.0 + self.bonuses.speed)
    }

    fn simulate(&mut self, dt: f32) {
        let typing = self.chat_input.is_some() || self.menu_open;
        let key = |k: KeyCode| !typing && is_key_down(k);
        let lmb = self.lmb.is_some();
        let rmb = self.rmb.is_some();
        let factor = self.movement_factor();
        let alive = self.my_view().is_some_and(|v| !v.dead);

        // Arrow keys turn the camera (and with it, the character).
        if alive {
            if key(KeyCode::Left) {
                self.cam_yaw += TURN_SPEED * dt;
            }
            if key(KeyCode::Right) {
                self.cam_yaw -= TURN_SPEED * dt;
            }
        }
        let mut fwd = 0.0;
        if key(KeyCode::W) || key(KeyCode::Up) || (lmb && rmb) {
            fwd += 1.0;
        }
        if key(KeyCode::S) || key(KeyCode::Down) {
            fwd -= 1.0;
        }
        // A and D strafe.
        let mut strafe = 0.0;
        if key(KeyCode::A) {
            strafe -= 1.0;
        }
        if key(KeyCode::D) {
            strafe += 1.0;
        }
        let wants_move = (fwd != 0.0 || strafe != 0.0) && alive;
        // Moving (or steering with the right button) turns you to face where
        // the camera looks.
        if alive && (rmb || wants_move || key(KeyCode::Left) || key(KeyCode::Right)) {
            self.yaw = self.cam_yaw;
        }
        let f = forward(self.yaw);
        let right = vec3(-f.z, 0.0, f.x);
        let dir = (f * fwd + right * strafe).normalize_or_zero();
        let speed = if fwd < 0.0 {
            BACKPEDAL_SPEED
        } else {
            RUN_SPEED
        } * factor;
        self.moving = dir != Vec3::ZERO && factor > 0.0;
        if self.moving {
            // Houses, trees, rocks and the like are solid, and so are the
            // vault's walls.
            let next = self.pos + dir * speed * dt;
            self.pos = if self.in_dungeon() {
                dungeon::resolve(next, 0.45)
            } else {
                self.colliders.resolve(next, 0.45)
            };
        }
        if key(KeyCode::Space) && self.grounded && factor > 0.0 {
            self.vel_y = JUMP_SPEED;
            self.grounded = false;
        }
        self.vel_y -= GRAVITY * dt;
        self.pos.y += self.vel_y * dt;
        let floor = terrain_height(self.pos.x, self.pos.z);
        if self.pos.y <= floor {
            self.pos.y = floor;
            self.vel_y = 0.0;
            self.grounded = true;
        }
        self.pos = clamp_to_world(self.pos);

        // Walking away from a merchant closes their window.
        if let Some(id) = self.windows.vendor
            && self
                .entities
                .get(&id)
                .is_none_or(|m| m.pos.distance(self.pos) > MERCHANT_RANGE + 2.0)
        {
            self.windows.vendor = None;
        }

        // Everyone else glides towards where the server says they are.
        let smooth = 1.0 - (-12.0 * dt).exp();
        let my_id = self.my_id;
        for e in self.entities.values_mut() {
            if Some(e.view.id) == my_id {
                e.pos = self.pos;
                e.yaw = self.yaw;
                if self.moving {
                    e.walk += dt * 9.0;
                }
            } else {
                if e.pos.distance(e.view.pos) > 15.0 {
                    e.pos = e.view.pos;
                }
                let before = e.pos;
                e.pos += (e.view.pos - e.pos) * smooth;
                e.yaw += wrap_angle(e.view.yaw - e.yaw) * smooth;
                e.walk += before.distance(e.pos) * 1.3;
            }
            e.swing += dt * 2.5;
            e.hurt += dt;
        }

        for f in &mut self.floats {
            f.age += dt;
            if let Some(e) = self.entities.get(&f.entity) {
                f.anchor = e.pos;
            }
        }
        self.floats.retain(|f| f.age < 1.4);
        for e in &mut self.errors {
            e.1 += dt;
        }
        self.errors.retain(|e| e.1 < 2.5);
        if let Some((_, _, t)) = &mut self.banner {
            *t += dt;
            if *t > 5.0 {
                self.banner = None;
            }
        }
        if let Some((_, t)) = &mut self.cast_flash {
            *t += dt;
            if *t > 1.0 {
                self.cast_flash = None;
            }
        }
        let anchors: std::collections::HashMap<EntityId, crate::vfx::Anchor> = self
            .entities
            .keys()
            .filter_map(|id| self.anchor(*id).map(|a| (*id, a)))
            .collect();
        self.vfx.update(dt, |id| anchors.get(&id).copied());
        if let Some((_, t)) = &mut self.quest_flash {
            *t += dt;
            if *t > 3.0 {
                self.quest_flash = None;
            }
        }
        if let Some(id) = self.windows.quest_giver
            && self
                .entities
                .get(&id)
                .is_none_or(|g| g.pos.distance(self.pos) > MERCHANT_RANGE + 2.0)
        {
            self.windows.quest_giver = None;
        }
    }

    fn send_movement(&mut self, dt: f32) {
        self.send_timer += dt;
        self.heartbeat += dt;
        if self.send_timer < 0.05 || self.my_id.is_none() {
            return;
        }
        self.send_timer = 0.0;
        let (pos, yaw, moving) = self.last_sent;
        let changed =
            pos.distance(self.pos) > 0.01 || (yaw - self.yaw).abs() > 0.01 || moving != self.moving;
        if changed || self.heartbeat > 1.0 {
            self.heartbeat = 0.0;
            self.last_sent = (self.pos, self.yaw, self.moving);
            self.send(ClientMsg::Move {
                pos: self.pos,
                yaw: self.yaw,
                moving: self.moving,
            });
        }
    }

    // ---- Drawing ----

    pub fn camera(&self) -> Camera3D {
        let head = self.pos + Vec3::Y * 1.8;
        let (sp, cp) = self.cam_pitch.sin_cos();
        let dir = vec3(self.cam_yaw.sin() * cp, -sp, self.cam_yaw.cos() * cp);
        let mut eye = head - dir * self.cam_dist;
        if self.in_dungeon() {
            // Stay under the ceiling and on this side of the walls.
            let mut d = 0.0;
            while d < self.cam_dist && !dungeon::blocked(head - dir * (d + 0.25), 0.3) {
                d += 0.25;
            }
            eye = head - dir * d;
            eye.y = eye.y.min(dungeon::WALL_HEIGHT - 0.4);
        }
        let floor = terrain_height(eye.x, eye.z) + 0.4;
        eye.y = eye.y.max(floor);
        Camera3D {
            position: eye,
            target: head,
            up: Vec3::Y,
            fovy: 1.0,
            z_near: 0.1,
            z_far: 1500.0,
            ..Default::default()
        }
    }

    fn draw_world(&mut self, scene: &Scene) {
        match self.place {
            Place::Dungeon(i) => scene.draw_dungeon(i),
            Place::Zone(z) => scene.draw(z),
        }
        let b = &mut self.batch;

        if let Some(t) = self.target.and_then(|t| self.entities.get(&t)) {
            let color = hud::reaction_color(&t.view, self.class);
            b.ground_ring(t.pos, render::model_radius(t.view.kind), 0.14, color);
        }
        // Marked ground: a red circle, filling in from the middle until it
        // lands.
        for h in &self.hazards {
            let pulse = 0.75 + 0.25 * (self.time * 8.0).sin();
            let edge = Color::new(1.0, 0.15, 0.1, 0.9 * pulse);
            b.ground_ring(h.pos, h.radius, 0.3, edge);
            let filled = (1.0 - h.remaining / h.total.max(0.01)).clamp(0.0, 1.0);
            let fill = Color::new(1.0, 0.3, 0.1, 0.55);
            let mut r = 0.35;
            while r < h.radius * filled {
                b.ground_ring(h.pos, r, 0.35, fill);
                r += 0.7;
            }
        }
        for e in self.entities.values() {
            let mine = Some(e.view.id) == self.my_id;
            let airborne = if mine {
                !self.grounded
            } else {
                e.pos.y > terrain_height(e.pos.x, e.pos.z) + 0.35
            };
            let pose = Pose {
                walk: e.walk,
                moving: e.view.moving || (mine && self.moving),
                casting: e.view.cast.is_some(),
                swing: e.swing.min(1.0),
                combo: e.combo,
                dead: e.view.dead,
                airborne,
                hurt: (1.0 - e.hurt / 0.35).max(0.0),
                time: self.time + e.view.id as f32,
            };
            render::draw_model(b, &e.look(), e.pos, e.yaw, pose);
            // Corpses you can loot sparkle.
            if e.view.lootable {
                for k in 0..4 {
                    let a = self.time * 2.0 + k as f32 * 1.57;
                    let p = e.pos
                        + vec3(
                            a.cos() * 0.7,
                            0.6 + (self.time * 3.0 + k as f32).sin() * 0.25,
                            a.sin() * 0.7,
                        );
                    b.glow_sphere(p, 0.07, Color::new(1.0, 0.88, 0.35, 1.0));
                }
            }
        }
        let anchors: std::collections::HashMap<EntityId, crate::vfx::Anchor> = self
            .entities
            .values()
            .map(|e| {
                (
                    e.view.id,
                    crate::vfx::Anchor {
                        pos: e.pos,
                        height: render::model_height(e.view.kind, e.view.appearance),
                        yaw: e.yaw,
                    },
                )
            })
            .collect();
        for e in self.entities.values() {
            let Some(a) = anchors.get(&e.view.id).copied() else {
                continue;
            };
            if e.view.dead {
                continue;
            }
            self.vfx.draw_auras(b, a, &e.view.auras, self.time);
            if let Some(cast) = &e.view.cast {
                self.vfx.draw_casting(
                    a,
                    cast.ability,
                    cast.elapsed / cast.total.max(0.01),
                    self.time,
                );
            }
        }
        self.vfx.draw(b, self.time, |id| anchors.get(&id).copied());
        if matches!(self.place, Place::Dungeon(_)) {
            scene.draw_dungeon_effects(b, self.time);
            b.flush();
        } else {
            scene.draw_effects(self.zone, b, self.time, self.pos);
            b.flush();
            scene.draw_water(self.zone);
        }
        self.vfx.draw_glows();
    }
}

/// An angle wrapped to `-PI..PI`.
pub fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

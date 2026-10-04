//! The in-game state: talks to the server, moves the player, handles input
//! and draws the 3D world.

use std::collections::HashMap;

use macroquad::prelude::*;
use shared::data::*;
use shared::net::Connection;
use shared::protocol::*;
use shared::world::*;

use crate::hud::{self, Layout};
use crate::render::{self, Batch, Pose, Scene};

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

struct Projectile {
    from: EntityId,
    to: EntityId,
    start: Vec3,
    color: Color,
    age: f32,
}

/// A ring that grows (spells around the caster) or rises (heals, level ups).
struct RingEffect {
    entity: EntityId,
    radius: f32,
    color: Color,
    rising: bool,
    age: f32,
    duration: f32,
}

/// A mouse button held down, and whether it has been dragged (camera control)
/// or is still a click.
#[derive(Clone, Copy)]
struct Press {
    start: Vec2,
    dragged: bool,
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
    last_mouse: Vec2,

    pub chat: Vec<ChatLine>,
    pub chat_input: Option<String>,
    pub errors: Vec<(String, f32)>,
    pub floats: Vec<FloatText>,
    pub banner: Option<(String, f32)>,
    /// "Interrupted" and similar, shown on the cast bar.
    pub cast_flash: Option<(String, f32)>,
    projectiles: Vec<Projectile>,
    rings: Vec<RingEffect>,
    pub show_help: bool,
    pub menu_open: bool,
    pub time: f32,
    batch: Batch,
}

pub enum Outcome {
    Continue,
    /// Back to the main menu, with a message.
    Leave(String),
}

impl Game {
    pub fn new(mut conn: Connection, name: &str, class: Class) -> std::io::Result<Self> {
        conn.send(&ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: name.to_string(),
            class,
        })?;
        let (mx, my) = mouse_position();
        let mut game = Self {
            conn,
            class,
            my_id: None,
            entities: HashMap::new(),
            me: SelfView::default(),
            target: None,
            pos: ground(GRAVEYARD.x, GRAVEYARD.y),
            yaw: 0.0,
            vel_y: 0.0,
            grounded: true,
            moving: false,
            send_timer: 0.0,
            last_sent: (Vec3::ZERO, 0.0, false),
            heartbeat: 0.0,
            cam_yaw: 0.0,
            cam_pitch: 0.35,
            cam_dist: 9.0,
            lmb: None,
            rmb: None,
            last_mouse: vec2(mx, my),
            chat: Vec::new(),
            chat_input: None,
            errors: Vec::new(),
            floats: Vec::new(),
            banner: None,
            cast_flash: None,
            projectiles: Vec::new(),
            rings: Vec::new(),
            show_help: true,
            menu_open: false,
            time: 0.0,
            batch: Batch::new(),
        };
        game.system("Welcome! Press H to show or hide the controls.");
        Ok(game)
    }

    pub fn my_view(&self) -> Option<&EntityView> {
        self.my_id
            .and_then(|id| self.entities.get(&id))
            .map(|e| &e.view)
    }

    pub fn target_ent(&self) -> Option<&Ent> {
        self.target.and_then(|t| self.entities.get(&t))
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

    fn error(&mut self, text: &str) {
        self.errors.retain(|(t, _)| t != text);
        self.errors.push((text.to_string(), 0.0));
        if self.errors.len() > 3 {
            self.errors.remove(0);
        }
    }

    pub fn frame(&mut self, scene: &Scene) -> Outcome {
        let dt = get_frame_time().min(0.1);
        self.time += dt;
        if let Err(reason) = self.network() {
            return Outcome::Leave(reason);
        }
        let layout = Layout::new();
        if let Some(outcome) = self.input(&layout) {
            return outcome;
        }
        self.simulate(dt);
        self.send_movement(dt);

        clear_background(Color::new(0.55, 0.75, 0.95, 1.0));
        let cam = self.camera();
        set_camera(&cam);
        self.draw_world(scene);
        set_default_camera();
        hud::draw(self, &layout, &cam);
        Outcome::Continue
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
                self.pos = pos;
                self.yaw = yaw;
                self.cam_yaw = yaw;
                self.vel_y = 0.0;
            }
            ServerMsg::Snapshot(snap) => {
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
                                },
                            );
                        }
                    }
                }
                self.entities.retain(|id, _| seen.contains(id));
                self.me = snap.me;
                if self.target.is_some_and(|t| !self.entities.contains_key(&t)) {
                    self.target = None;
                }
            }
            ServerMsg::Event(event) => self.event(event),
        }
        Ok(())
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
                }
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
                let color = hud::school_color(a.school);
                if a.projectile {
                    if let (Some(to), Some(from)) = (target, self.entities.get(&caster)) {
                        self.projectiles.push(Projectile {
                            from: caster,
                            to,
                            start: from.pos + Vec3::Y * 1.5,
                            color,
                            age: 0.0,
                        });
                    }
                } else if let Targeting::AroundCaster(radius) = a.targeting {
                    self.rings.push(RingEffect {
                        entity: caster,
                        radius,
                        color,
                        rising: false,
                        age: 0.0,
                        duration: 0.45,
                    });
                } else if a.range <= MELEE_RANGE
                    && a.targeting == Targeting::Enemy
                    && let Some(e) = self.entities.get_mut(&caster)
                {
                    e.swing = 0.0;
                }
                if a.effects.iter().any(|e| {
                    matches!(
                        e,
                        Effect::Heal { .. }
                            | Effect::Aura {
                                kind: AuraKind::Hot { .. } | AuraKind::Absorb(_),
                                ..
                            }
                    )
                }) {
                    self.rings.push(RingEffect {
                        entity: target.unwrap_or(caster),
                        radius: 0.9,
                        color,
                        rising: true,
                        age: 0.0,
                        duration: 0.8,
                    });
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
                self.rings.push(RingEffect {
                    entity: id,
                    radius: 1.3,
                    color: Color::new(1.0, 0.85, 0.3, 1.0),
                    rising: true,
                    age: 0.0,
                    duration: 1.5,
                });
                if Some(id) == me {
                    self.banner = Some((format!("Level {level}!"), 0.0));
                    self.system(&format!("Congratulations, you have reached level {level}!"));
                }
            }
            GameEvent::Xp { amount, from } => {
                self.chat.push(ChatLine {
                    text: format!("{from} dies, you gain {amount} experience."),
                    color: Color::new(0.75, 0.6, 1.0, 1.0),
                });
            }
            GameEvent::Error(text) => self.error(&text),
            GameEvent::Chat { from, text } => self.chat.push(ChatLine {
                text: format!("[{from}]: {text}"),
                color: WHITE,
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

    fn use_slot(&mut self, slot: usize) {
        let ability = self.class.abilities()[slot];
        if self.my_view().is_some_and(|v| v.dead) {
            self.error("You are dead.");
            return;
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
                let h = render::model_height(e.view.kind);
                let r = render::model_radius(e.view.kind);
                let bottom = hud::project(cam, e.pos)?;
                let top = hud::project(cam, e.pos + Vec3::Y * h)?;
                let height = (bottom.y - top.y).max(8.0);
                let half_width = (height * r / h).max(12.0);
                let inside = mouse.y >= top.y - 4.0
                    && mouse.y <= bottom.y + 4.0
                    && (mouse.x - bottom.x).abs() <= half_width;
                inside.then(|| (e.pos.distance(cam.position), e.view.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id)
    }

    fn input(&mut self, layout: &Layout) -> Option<Outcome> {
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
            } else if self.target.is_some() {
                self.set_target(None);
            } else {
                self.menu_open = true;
            }
        }

        let left_pressed = is_mouse_button_pressed(MouseButton::Left);
        if self.menu_open {
            self.lmb = None;
            self.rmb = None;
            if left_pressed {
                if layout.menu_buttons[0].contains(mouse) {
                    self.menu_open = false;
                } else if layout.menu_buttons[1].contains(mouse) {
                    return Some(Outcome::Leave("Logged out.".into()));
                } else if layout.menu_buttons[2].contains(mouse) {
                    std::process::exit(0);
                }
            }
            return None;
        }

        // Clicks on the interface.
        let over_ui = layout.blocks(mouse, self.target.is_some(), dead);
        if left_pressed && over_ui {
            if let Some(slot) = layout.hotbar.iter().position(|r| r.contains(mouse)) {
                self.use_slot(slot);
            } else if layout.player_frame.contains(mouse) {
                self.set_target(self.my_id);
            } else if dead && layout.release_button.contains(mouse) {
                self.send(ClientMsg::ReleaseSpirit);
            }
        }

        // Mouse buttons in the world: drag to look around, click to target.
        if left_pressed && !over_ui {
            self.lmb = Some(Press {
                start: mouse,
                dragged: false,
            });
        }
        if is_mouse_button_pressed(MouseButton::Right) && !over_ui {
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
        if self.lmb.is_some_and(|p| p.dragged) || self.rmb.is_some_and(|p| p.dragged) {
            self.cam_yaw -= delta.x * MOUSE_SENSITIVITY;
            self.cam_pitch = (self.cam_pitch + delta.y * MOUSE_SENSITIVITY).clamp(-0.4, 1.35);
        }
        if is_mouse_button_released(MouseButton::Left)
            && let Some(press) = self.lmb.take()
            && !press.dragged
            && let Some(id) = self.pick(&cam, mouse)
        {
            self.set_target(Some(id));
        }
        if is_mouse_button_released(MouseButton::Right)
            && let Some(press) = self.rmb.take()
            && !press.dragged
            && let Some(id) = self.pick(&cam, mouse)
        {
            self.set_target(Some(id));
            if self
                .entities
                .get(&id)
                .is_some_and(|e| self.is_hostile(&e.view) && !e.view.dead)
            {
                self.send(ClientMsg::StartAttack);
            }
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
        if is_key_pressed(KeyCode::T) {
            if self.me.auto_attacking {
                self.send(ClientMsg::StopAttack);
            } else {
                self.send(ClientMsg::StartAttack);
            }
        }
        if is_key_pressed(KeyCode::H) {
            self.show_help = !self.show_help;
        }
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter) {
            self.chat_input = Some(String::new());
        } else if is_key_pressed(KeyCode::Slash) {
            self.chat_input = Some("/".into());
        }
        None
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
                        AuraKind::Slow(f) => factor = factor.min(*f),
                        _ => {}
                    }
                }
            }
        }
        factor
    }

    fn simulate(&mut self, dt: f32) {
        let typing = self.chat_input.is_some() || self.menu_open;
        let key = |k: KeyCode| !typing && is_key_down(k);
        let lmb = self.lmb.is_some();
        let rmb = self.rmb.is_some();
        let factor = self.movement_factor();

        let mut turning = false;
        if rmb {
            self.yaw = self.cam_yaw;
        } else if factor > 0.0 || self.my_view().is_some_and(|v| !v.dead) {
            if key(KeyCode::A) || key(KeyCode::Left) {
                self.yaw += TURN_SPEED * dt;
                turning = true;
            }
            if key(KeyCode::D) || key(KeyCode::Right) {
                self.yaw -= TURN_SPEED * dt;
                turning = true;
            }
        }
        let mut fwd = 0.0;
        if key(KeyCode::W) || key(KeyCode::Up) || (lmb && rmb) {
            fwd += 1.0;
        }
        if key(KeyCode::S) || key(KeyCode::Down) {
            fwd -= 1.0;
        }
        let mut strafe = 0.0;
        if key(KeyCode::Q) || (rmb && key(KeyCode::A)) {
            strafe -= 1.0;
        }
        if key(KeyCode::E) || (rmb && key(KeyCode::D)) {
            strafe += 1.0;
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
            self.pos += dir * speed * dt;
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
        // Swing the camera back behind the player as they move.
        if (self.moving || turning) && !lmb && !rmb {
            let diff = wrap_angle(self.yaw - self.cam_yaw);
            self.cam_yaw += diff * (1.0 - (-5.0 * dt).exp());
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
        if let Some((_, t)) = &mut self.banner {
            *t += dt;
            if *t > 3.0 {
                self.banner = None;
            }
        }
        if let Some((_, t)) = &mut self.cast_flash {
            *t += dt;
            if *t > 1.0 {
                self.cast_flash = None;
            }
        }
        for p in &mut self.projectiles {
            p.age += dt;
        }
        let entities = &self.entities;
        self.projectiles.retain(|p| {
            p.age < 1.0
                && entities
                    .get(&p.to)
                    .is_some_and(|t| p.start.distance(t.pos) > p.age * 35.0)
        });
        for r in &mut self.rings {
            r.age += dt;
        }
        self.rings.retain(|r| r.age < r.duration);
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
        scene.draw();
        let b = &mut self.batch;

        if let Some(t) = self.target.and_then(|t| self.entities.get(&t)) {
            let color = hud::reaction_color(&t.view, self.class);
            b.ground_ring(t.pos, render::model_radius(t.view.kind), 0.14, color);
        }
        for e in self.entities.values() {
            let pose = Pose {
                walk: e.walk,
                moving: e.view.moving || (Some(e.view.id) == self.my_id && self.moving),
                casting: e.view.cast.is_some(),
                swing: e.swing.min(1.0),
                dead: e.view.dead,
                time: self.time + e.view.id as f32,
            };
            render::draw_model(b, e.view.kind, e.pos, e.yaw, pose);
        }
        for p in &self.projectiles {
            let Some(to) = self.entities.get(&p.to) else {
                continue;
            };
            let end = to.pos + Vec3::Y * render::model_height(to.view.kind) * 0.6;
            let dist = p.start.distance(end).max(0.1);
            let t = (p.age * 35.0 / dist).min(1.0);
            let at = p.start.lerp(end, t);
            b.sphere(at, 0.25, p.color);
            b.sphere(
                at - (end - p.start).normalize_or_zero() * 0.35,
                0.15,
                p.color,
            );
            let _ = p.from;
        }
        for r in &self.rings {
            let Some(e) = self.entities.get(&r.entity) else {
                continue;
            };
            let t = r.age / r.duration;
            let mut color = r.color;
            color.a = 1.0 - t;
            if r.rising {
                for i in 0..3 {
                    let y = (t * 2.2 + i as f32 * 0.5) % 2.2;
                    b.air_ring(e.pos + Vec3::Y * y, r.radius, 0.08, color);
                }
            } else {
                b.air_ring(e.pos + Vec3::Y * 0.3, r.radius * t.sqrt(), 0.5, color);
            }
        }
        b.flush();
        scene.draw_water();
    }
}

/// An angle wrapped to `-PI..PI`.
pub fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

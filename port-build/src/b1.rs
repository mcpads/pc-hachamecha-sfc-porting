use crate::{Result, SOURCE, sha};
use serde::{Deserialize, Serialize};
const DELTAS: [(i16, i16); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
#[derive(Clone, Deserialize)]
struct Ray {
    turns: Vec<u8>,
    slots: Vec<usize>,
}
#[derive(Clone, Deserialize)]
pub(crate) struct Event {
    pub(crate) id: u8,
    x: u8,
    y: u8,
    direction_mask: u8,
    pub(crate) kind: u8,
    script_va: u32,
}
#[derive(Clone, Deserialize)]
pub(crate) struct World {
    schema: u8,
    source_sha256: String,
    floor: u8,
    width: u8,
    height: u8,
    pub(crate) initial_state: [u8; 4],
    cells: Vec<u16>,
    direction_masks: [u8; 4],
    view_rays: Vec<Ray>,
    view_slot_zero: u8,
    view_masks: [u16; 4],
    events: Vec<Event>,
}
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    Forward,
    Left,
    Right,
    Around,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Move {
    before: [u8; 4],
    action: Action,
    after: [u8; 4],
    effect: u8,
}
#[derive(Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct Reference {
    schema: u8,
    source_sha256: String,
    views: Vec<Vec<u8>>,
    moves: Vec<Move>,
}
impl World {
    pub(crate) fn floor_map(&self, floor: u8, cells: Vec<u16>) -> Result<Self> {
        if cells.len() != 64 {
            return Err("Invalid floor map".into());
        }
        let mut map = self.clone();
        map.floor = floor;
        map.cells = cells;
        map.events.clear();
        Ok(map)
    }
    pub(crate) fn cells(&self) -> &[u16] {
        &self.cells
    }
    pub(crate) fn rays(&self) -> (crate::sfc_floor_walls::Rays, [u16; 4], u8) {
        (
            self.view_rays
                .iter()
                .map(|r| (r.turns.clone(), r.slots.clone()))
                .collect(),
            self.view_masks,
            self.view_slot_zero,
        )
    }
    pub(crate) fn validate(&self) -> Result<()> {
        if self.schema != 1
            || self.source_sha256 != SOURCE
            || (self.floor, self.width, self.height) != (1, 8, 8)
            || self.cells.len() != 64
            || self.initial_state != [1, 3, 7, 0]
            || self.direction_masks != [1, 2, 8, 4]
            || self.view_masks != [15, 240, 3840, 61440]
            || self.view_slot_zero != 1
            || self.view_rays.len() != 11
            || self.events.len() != 18
            || self.view_rays.iter().any(|r| {
                r.turns.len() != r.slots.len()
                    || r.turns.len() > 5
                    || r.turns.iter().any(|t| *t > 3)
                    || r.slots.iter().any(|s| !(1..20).contains(s))
            })
        {
            return Err("Invalid B1 navigation definition".into());
        }
        for e in &self.events {
            if e.x >= 8
                || e.y >= 8
                || e.direction_mask == 0
                || e.direction_mask > 15
                || e.kind > 15
                || e.script_va == 0
            {
                return Err("Invalid B1 event binding".into());
            }
        }
        Ok(())
    }
    fn index(&self, state: [u8; 4]) -> Result<usize> {
        let [floor, x, y, direction] = state;
        if floor != self.floor || x >= 8 || y >= 8 || direction >= 4 {
            return Err("B1 state outside map".into());
        }
        Ok(usize::from(y) * 8 + usize::from(x))
    }
    pub(crate) fn event(&self, state: [u8; 4]) -> Option<&Event> {
        self.events.iter().find(|e| {
            e.x == state[1]
                && e.y == state[2]
                && e.direction_mask & self.direction_masks[state[3] as usize] != 0
        })
    }
    pub(crate) fn step(&self, mut state: [u8; 4], action: Action) -> Result<([u8; 4], u8)> {
        let cell = self.index(state)?;
        let mut effect = 0;
        match action {
            Action::Forward => {
                let edge = (self.cells[cell] >> (state[3] * 4)) & 15;
                if edge & 8 != 0 {
                    effect = 3;
                } else {
                    let (dx, dy) = DELTAS[state[3] as usize];
                    let (x, y) = (i16::from(state[1]) + dx, i16::from(state[2]) + dy);
                    if !(0..8).contains(&x) || !(0..8).contains(&y) {
                        return Err("Open edge leads outside B1".into());
                    }
                    state[1] = x as u8;
                    state[2] = y as u8;
                    effect = if edge & 7 == 0 { 1 } else { 2 };
                }
            }
            Action::Left => state[3] = (state[3] + 3) & 3,
            Action::Right => state[3] = (state[3] + 1) & 3,
            Action::Around => state[3] = (state[3] + 2) & 3,
        }
        Ok((state, effect))
    }
    pub(crate) fn view(&self, state: [u8; 4]) -> Result<[u8; 20]> {
        self.index(state)?;
        let mut slots = [0; 20];
        slots[0] = self.view_slot_zero;
        for ray in &self.view_rays {
            let mut current = state;
            for (&turn, &slot) in ray.turns.iter().zip(&ray.slots) {
                current[3] = (current[3] + turn) & 3;
                let cell = self.cells[self.index(current)?];
                if cell & self.view_masks[current[3] as usize] != 0 {
                    slots[slot] = ((cell >> (current[3] * 4)) & 15) as u8;
                    break;
                }
                let (dx, dy) = DELTAS[current[3] as usize];
                current[1] = u8::try_from(i16::from(current[1]) + dx)?;
                current[2] = u8::try_from(i16::from(current[2]) + dy)?;
            }
        }
        Ok(slots)
    }
}
pub fn compile(content: &crate::content::Content) -> Result<Vec<(String, Vec<u8>)>> {
    let bytes = content.read("b1-world.json")?;
    let world: World = serde_json::from_slice(&bytes)?;
    world.validate()?;
    let mut output = b"HBN1".to_vec();
    output.extend(world.initial_state);
    let mut states = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            for direction in 0..4 {
                let state = [1, x, y, direction];
                let view = world.view(state)?;
                let event = world.event(state);
                output.push(event.map_or(255, |e| e.id));
                output.push(event.map_or(0, |e| e.kind));
                output.extend(view);
                for action in [Action::Forward, Action::Left, Action::Right, Action::Around] {
                    let (after, effect) = world.step(state, action)?;
                    output.extend([after[1], after[2], after[3], effect]);
                }
                states.push(serde_json::json!(
                    { "state" : state, "event" : event.map(| e | e.id),
                    "script_va" : event.map(| e | e.script_va), "view" : view }
                ));
            }
        }
    }
    let report = serde_json::json!(
        { "schema" : 1, "scope" :
        "B1 static navigation and view selection; not SFC gameplay", "content_sha256" :
        sha(& bytes), "payload_sha256" : sha(& output), "states" : states, "binary" : {
        "magic" : "HBN1", "header_bytes" : 8, "record_bytes" : 38, "order" :
        "y,x,direction", "record" :
        "event u8, kind u8, view[20], forward/left/right/around [x,y,direction,effect]"
        }, "limits" :
        ["Resolved single-action input only; menu and interaction ownership not yet connected.",
        "Event mutation, flags, inventory and shared choice service remain unimplemented.",
        "This is initial-state data; runtime map mutations must recompute view and movement."]
        }
    );
    Ok(vec![
        ("b1-navigation.bin".into(), output),
        (
            "b1-navigation-build.json".into(),
            serde_json::to_vec_pretty(&report)?,
        ),
    ])
}

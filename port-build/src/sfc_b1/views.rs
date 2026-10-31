use super::*;
pub(super) fn picture_jobs_of(floor: &crate::floor_events::Floor) -> Result<Vec<PictureJob>> {
    let mut jobs = Vec::new();
    for event in &floor.events {
        let Some(script) = event.script else {
            continue;
        };
        let creations = floor.creations(script);
        let sliding = floor.slid_slots(script);
        let mut sets: Vec<Vec<u16>> = Vec::new();
        for (i, (at_a, a, slot_a)) in creations.iter().enumerate() {
            sets.push(vec![*a]);
            for (at_b, b, slot_b) in &creations[i + 1..] {
                let registered = matches!(
                    (slot_a, slot_b), (Some(x), Some(y)) if x != y
                );
                let ordered =
                    floor.dominates(script, *at_a, *at_b) || floor.dominates(script, *at_b, *at_a);
                let in_turn = slot_a != slot_b
                    && (floor.leads_to(*at_a, *at_b) || floor.leads_to(*at_b, *at_a));
                if a != b && (registered || ordered || in_turn) {
                    sets.push(vec![*a, *b]);
                }
            }
        }
        sets.sort();
        sets.dedup();
        let here: Vec<[u8; 4]> = (0..4u8)
            .filter(|&direction| {
                let mask = crate::floor_events::DIRECTION_MASKS[usize::from(direction)];
                let first = floor
                    .events
                    .iter()
                    .find(|e| e.x == event.x && e.y == event.y && e.direction_mask & mask != 0);
                first.map(|e| e.id) == Some(event.id)
            })
            .map(|direction| [floor.id, event.x, event.y, direction])
            .collect();
        let arrivals = floor.arrivals(script);
        for set in &sets {
            let mut states: Vec<[u8; 4]> = Vec::new();
            for (at, ..) in creations.iter().filter(|(_, o, _)| set.contains(o)) {
                let before: Vec<_> = arrivals
                    .iter()
                    .filter(|(change, ..)| floor.dominates(script, *change, *at))
                    .collect();
                let last = before.iter().find(|(c, ..)| {
                    before
                        .iter()
                        .all(|(d, ..)| d == c || floor.dominates(script, *d, *c))
                });
                let arrived: Vec<[u8; 4]> = match last {
                    Some((_, to, [x, y, crate::floor_events::KEEP_FACING])) => {
                        (0..4).map(|d| [*to, *x, *y, d]).collect()
                    }
                    Some((_, to, [x, y, d])) => vec![[*to, *x, *y, *d]],
                    None => here.clone(),
                };
                for state in arrived {
                    if !states.contains(&state) {
                        states.push(state);
                    }
                }
            }
            for state in states {
                let entries: Vec<u32> = set
                    .iter()
                    .map(|&o| floor.pictures[usize::from(o)])
                    .collect();
                let mut ids = set
                    .iter()
                    .map(|&o| u8::try_from(o + 1))
                    .collect::<std::result::Result<Vec<u8>, _>>()?;
                ids.sort();
                if ids.iter().any(|&i| i >= crate::sfc_field_events::SLIDE_KEY) {
                    return Err("Picture id reaches the slide keys".into());
                }
                let key = match ids[..] {
                    [one] => [0, one],
                    [a, b] => [a, b],
                    _ => return Err("More than two pictures together".into()),
                };
                let slid = match set[..] {
                    [one] => creations.iter().find_map(|(_, o, slot)| {
                        let slot = slot.filter(|_| *o == one)?;
                        sliding
                            .iter()
                            .find(|(s, _)| *s == slot)
                            .map(|(_, axis)| *axis)
                    }),
                    _ => None,
                };
                let slides = slid.map_or(0, |_| crate::sfc_field_events::SLIDE_STEPS);
                for step in 0..=slides {
                    jobs.push(PictureJob {
                        state,
                        entries: entries.clone(),
                        shift: {
                            let d =
                                i32::from(step) * i32::from(crate::sfc_field_events::SLIDE_PIXELS);
                            if slid == Some(1) { [0, d] } else { [d, 0] }
                        },
                        key: if step == 0 {
                            key
                        } else {
                            [crate::sfc_field_events::SLIDE_KEY + step, key[1]]
                        },
                    });
                }
            }
        }
    }
    Ok(jobs)
}
pub(super) fn draws_of(def: &Definition, slots: &[u8; 20], parity: u8) -> Result<Vec<Draw>> {
    let mut draws = vec![Draw {
        slot: 15,
        source: def.backdrop_rectangles[usize::from(parity)],
        destination: [0, 0, 224, 240],
        transparent: false,
    }];
    for placement in &def.placements {
        let value = *slots.get(placement.slot).ok_or("Wall slot out of bounds")?;
        if value == 0 {
            continue;
        }
        let [l, t, r, b] = placement.source;
        let [dx, dy] = placement.destination;
        draws.push(Draw {
            slot: def.resource_slots[usize::from(value)],
            source: [l, t, r, b],
            destination: [dx, dy, dx + r - l, dy + b - t],
            transparent: true,
        });
    }
    Ok(draws)
}
pub(super) fn wall_variants(
    cells: &[u16],
    choices: &[crate::floor_events::WallChoice],
    x: u8,
    y: u8,
) -> Result<Vec<Vec<u16>>> {
    const DELTAS: [(i8, i8); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];
    let near: Vec<_> = choices
        .iter()
        .filter(|c| {
            c.alternatives.iter().flatten().any(|&((cell, _, _), _)| {
                let (cx, cy) = (cell % 8, cell / 8);
                cx.abs_diff(x) + cy.abs_diff(y) <= 7
            })
        })
        .collect();
    let count: usize = near.iter().map(|c| c.alternatives.len() + 1).product();
    if count > 4096 {
        return Err("Too many wall combinations near one state".into());
    }
    let mut variants = Vec::new();
    for mut index in 0..count {
        let mut cells = cells.to_vec();
        for choice in &near {
            let pick = index % (choice.alternatives.len() + 1);
            index /= choice.alternatives.len() + 1;
            if pick == 0 {
                continue;
            }
            for &((cell, facing, both), value) in &choice.alternatives[pick - 1] {
                let value = u16::from(value);
                let set = |cells: &mut Vec<u16>, cell: usize, facing: u8| {
                    let shift = facing * 4;
                    cells[cell] = (cells[cell] & !(15 << shift)) | value << shift;
                };
                set(&mut cells, usize::from(cell), facing);
                if both {
                    let (dx, dy) = DELTAS[usize::from(facing)];
                    let (nx, ny) = (
                        (cell % 8).wrapping_add_signed(dx),
                        (cell / 8).wrapping_add_signed(dy),
                    );
                    if nx < 8 && ny < 8 {
                        set(&mut cells, usize::from(ny * 8 + nx), (facing + 2) & 3);
                    }
                }
            }
        }
        variants.push(cells);
    }
    Ok(variants)
}

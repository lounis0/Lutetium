//! Precise generational tracing garbage collector for the bytecode VM.
//!
//! Design (see docs/RUNTIME_ARCHITECTURE.md):
//!
//! * Objects live in a slot table; `Ref(u32)` handles index the table, so
//!   references are stable across sweeping and promotion.
//! * Two generations. Allocation targets the young generation; survivors of
//!   two minor collections are promoted. Major collections sweep both.
//! * Root discovery is precise by construction: the VM hands the collector
//!   every live register vector; container contents are reached through the
//!   heap itself. No stack scanning.
//! * A sticky remembered set records old objects that may reference young
//!   objects (write barrier + promotion-time scan).
//! * Strings are `Rc<str>` (immutable, cycle-free) and are not traced;
//!   mutable containers are the traced objects.

use crate::vm::Value;

pub type Ref = u32;

#[derive(Debug, Clone)]
pub enum Obj {
    List(Vec<Value>),
    /// Insertion-ordered key/value pairs (linear lookup; fine at 0.4 scale).
    Map(Vec<(Value, Value)>),
}

impl Obj {
    fn bytes(&self) -> usize {
        match self {
            Obj::List(v) => 32 + v.len() * 16,
            Obj::Map(v) => 32 + v.len() * 32,
        }
    }

    fn children(&self) -> Vec<Value> {
        match self {
            Obj::List(v) => v.clone(),
            Obj::Map(v) => v
                .iter()
                .flat_map(|(k, val)| [k.clone(), val.clone()])
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gen {
    Young,
    Old,
}

struct GcObject {
    obj: Obj,
    mark: bool,
    age: u8,
    gen: Gen,
    in_remembered: bool,
    bytes: usize,
}

struct Slot {
    obj: Option<GcObject>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GcStats {
    pub allocations: u64,
    pub objects_freed: u64,
    pub minor_collections: u32,
    pub major_collections: u32,
    pub promotions: u64,
    pub live_slots: usize,
    pub live_bytes: usize,
    /// Sum of object byte sizes at free time (approximate; header size
    /// included, external string payloads excluded).
    pub freed_bytes: usize,
}

pub struct Heap {
    slots: Vec<Slot>,
    free: Vec<Ref>,
    young: usize,
    old: usize,
    remembered: Vec<Ref>,
    young_threshold: usize,
    old_threshold: usize,
    pub stress: bool,
    stats: GcStats,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Heap {
        Heap {
            slots: Vec::new(),
            free: Vec::new(),
            young: 0,
            old: 0,
            remembered: Vec::new(),
            young_threshold: 1024,
            old_threshold: 8192,
            stress: false,
            stats: GcStats::default(),
        }
    }

    pub fn stats(&self) -> GcStats {
        GcStats {
            live_slots: self.young + self.old,
            live_bytes: self
                .slots
                .iter()
                .filter_map(|s| s.obj.as_ref())
                .map(|o| o.bytes)
                .sum(),
            ..self.stats
        }
    }

    /// Allocate into the young generation. The caller is responsible for
    /// triggering collections (`needs_collection` + `collect_minor`).
    pub fn alloc(&mut self, obj: Obj) -> Ref {
        self.stats.allocations += 1;
        let bytes = obj.bytes();
        let o = GcObject {
            obj,
            mark: false,
            age: 0,
            gen: Gen::Young,
            in_remembered: false,
            bytes,
        };
        let r = match self.free.pop() {
            Some(r) => {
                self.slots[r as usize] = Slot { obj: Some(o) };
                r
            }
            None => {
                self.slots.push(Slot { obj: Some(o) });
                (self.slots.len() - 1) as Ref
            }
        };
        self.young += 1;
        r
    }

    pub fn needs_minor(&self) -> bool {
        self.stress || self.young >= self.young_threshold
    }

    pub fn needs_major(&self) -> bool {
        self.old >= self.old_threshold
    }

    pub fn get(&self, r: Ref) -> &Obj {
        &self.slots[r as usize]
            .obj
            .as_ref()
            .expect("dangling heap ref")
            .obj
    }

    pub fn get_mut(&mut self, r: Ref) -> &mut Obj {
        &mut self.slots[r as usize]
            .obj
            .as_mut()
            .expect("dangling heap ref")
            .obj
    }

    /// Write barrier: record an old-generation object that may now reference
    /// a young object. Sticky: entries persist across collections (they are
    /// re-validated during marking).
    pub fn write_barrier(&mut self, r: Ref) {
        let needs = matches!(
            self.slots[r as usize].obj.as_ref(),
            Some(o) if o.gen == Gen::Old && !o.in_remembered
        );
        if needs {
            if let Some(o) = self.slots[r as usize].obj.as_mut() {
                o.in_remembered = true;
            }
            self.remembered.push(r);
        }
    }

    fn mark_value(&mut self, v: &mut Value, worklist: &mut Vec<Ref>, trace_old: bool) {
        let r = match v.gc_ref() {
            Some(r) => r,
            None => return,
        };
        let is_young = matches!(
            self.slots[r as usize].obj.as_ref().map(|o| o.gen),
            Some(Gen::Young)
        );
        if !trace_old && !is_young {
            return; // old objects are collected by major collections
        }
        let marked = self.slots[r as usize]
            .obj
            .as_ref()
            .map(|o| o.mark)
            .unwrap_or(true); // dangling (should not happen): treat as live
        if marked {
            return;
        }
        if let Some(o) = self.slots[r as usize].obj.as_mut() {
            o.mark = true;
        }
        worklist.push(r);
    }

    fn trace(&mut self, worklist: &mut Vec<Ref>, trace_old: bool) {
        while let Some(r) = worklist.pop() {
            let children = self.slots[r as usize]
                .obj
                .as_ref()
                .map(|o| o.obj.children())
                .unwrap_or_default();
            for mut c in children {
                self.mark_value(&mut c, worklist, trace_old);
            }
        }
    }

    /// Minor collection: `roots` yields every live register vector.
    pub fn collect_minor(&mut self, roots: &mut [&mut Vec<Value>]) {
        self.stats.minor_collections += 1;
        let mut worklist: Vec<Ref> = Vec::new();

        // Roots: all registers.
        for regs in roots.iter_mut() {
            for v in regs.iter_mut() {
                self.mark_value(v, &mut worklist, false);
            }
        }
        // Remembered old objects may reference young objects.
        let remembered = self.remembered.clone();
        for r in remembered {
            let children = self.slots[r as usize]
                .obj
                .as_ref()
                .map(|o| o.obj.children())
                .unwrap_or_default();
            for mut c in children {
                self.mark_value(&mut c, &mut worklist, false);
            }
        }
        self.trace(&mut worklist, false);

        // Sweep the young generation; promote survivors.
        for i in 0..self.slots.len() {
            // First, classify the slot immutably to avoid borrow conflicts.
            let (is_young, marked, still_has_young) = match self.slots[i].obj.as_ref() {
                Some(o) => (
                    o.gen == Gen::Young,
                    o.mark,
                    o.obj.children().iter().any(|c| {
                        c.gc_ref()
                            .map(|cr| {
                                self.slots[cr as usize]
                                    .obj
                                    .as_ref()
                                    .map(|o| o.gen == Gen::Young)
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    }),
                ),
                None => continue,
            };
            if !is_young {
                continue;
            }
            let obj = self.slots[i].obj.as_mut().unwrap();
            if marked {
                obj.mark = false;
                obj.age += 1;
                if obj.age >= 2 {
                    // promote; if it references young objects, remember it
                    obj.gen = Gen::Old;
                    self.young -= 1;
                    self.old += 1;
                    self.stats.promotions += 1;
                    let r = i as Ref;
                    if still_has_young && !obj.in_remembered {
                        obj.in_remembered = true;
                        self.remembered.push(r);
                    }
                }
            } else {
                // dead young object
                self.stats.objects_freed += 1;
                self.stats.freed_bytes += obj.bytes;
                self.young -= 1;
                self.slots[i] = Slot { obj: None };
                self.free.push(i as Ref);
            }
        }
    }

    /// Major (full) collection over both generations.
    pub fn collect_major(&mut self, roots: &mut [&mut Vec<Value>]) {
        self.stats.major_collections += 1;
        let mut worklist: Vec<Ref> = Vec::new();
        for regs in roots.iter_mut() {
            for v in regs.iter_mut() {
                self.mark_value(v, &mut worklist, true);
            }
        }
        self.trace(&mut worklist, true);

        for i in 0..self.slots.len() {
            let Some(obj) = self.slots[i].obj.as_mut() else {
                continue;
            };
            if obj.mark {
                obj.mark = false;
            } else {
                self.stats.objects_freed += 1;
                self.stats.freed_bytes += obj.bytes;
                if obj.gen == Gen::Young {
                    self.young -= 1;
                } else {
                    self.old -= 1;
                }
                self.slots[i] = Slot { obj: None };
                self.free.push(i as Ref);
            }
        }
        // Re-validate the remembered set: drop entries for dead objects,
        // keep old objects that still reference young ones.
        self.remembered.retain(|r| {
            let live = self.slots[*r as usize].obj.is_some();
            if !live {
                if let Some(o) = self.slots[*r as usize].obj.as_mut() {
                    o.in_remembered = false;
                }
            }
            live
        });
        for r in self.remembered.clone() {
            let still_young = self.slots[r as usize]
                .obj
                .as_ref()
                .map(|o| {
                    o.obj.children().iter().any(|c| {
                        c.gc_ref()
                            .map(|cr| {
                                self.slots[cr as usize]
                                    .obj
                                    .as_ref()
                                    .map(|o| o.gen == Gen::Young)
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false);
            if !still_young {
                if let Some(o) = self.slots[r as usize].obj.as_mut() {
                    o.in_remembered = false;
                }
            }
        }
        self.remembered.retain(|r| {
            self.slots[*r as usize]
                .obj
                .as_ref()
                .map(|o| o.in_remembered)
                .unwrap_or(false)
        });
    }

    /// Collect if thresholds (or stress mode) require it.
    pub fn maybe_collect(&mut self, roots: &mut [&mut Vec<Value>]) {
        if self.needs_major() {
            self.collect_major(roots);
        }
        if self.needs_minor() {
            self.collect_minor(roots);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_objects_are_reclaimed() {
        let mut h = Heap::new();
        let mut regs: Vec<Value> = vec![];
        {
            let r = h.alloc(Obj::List(vec![]));
            assert!(matches!(h.get(r), Obj::List(_)));
        } // ref dropped; nothing holds it
        let mut roots = vec![&mut regs];
        h.collect_minor(&mut roots);
        assert_eq!(h.stats().objects_freed, 1);
        assert_eq!(h.stats().live_slots, 0);
    }

    #[test]
    fn live_objects_survive() {
        let mut h = Heap::new();
        let mut regs: Vec<Value> = vec![Value::List(h.alloc(Obj::List(vec![Value::Int(7)])))];
        let mut roots = vec![&mut regs];
        h.collect_minor(&mut roots);
        h.collect_minor(&mut roots);
        h.collect_minor(&mut roots);
        assert_eq!(h.stats().objects_freed, 0);
        match h.get(0) {
            Obj::List(v) => assert_eq!(v[0], Value::Int(7)),
            _ => panic!(),
        }
    }

    #[test]
    fn objects_are_promoted_to_old_generation() {
        let mut h = Heap::new();
        let mut regs: Vec<Value> = vec![Value::List(h.alloc(Obj::List(vec![])))];
        let mut roots = vec![&mut regs];
        h.collect_minor(&mut roots);
        h.collect_minor(&mut roots);
        assert_eq!(h.stats().promotions, 1);
        assert_eq!(h.stats().live_slots, 1);
    }

    #[test]
    fn old_to_young_references_survive_minor_collection() {
        let mut h = Heap::new();
        // inner: young list; outer: list containing inner
        let inner = h.alloc(Obj::List(vec![Value::Int(42)]));
        let outer = h.alloc(Obj::List(vec![Value::List(inner)]));
        let mut regs: Vec<Value> = vec![Value::List(outer)];
        let mut roots = vec![&mut regs];
        // promote both by two minor collections (barrier/promotion keeps
        // old->young references visible)
        h.collect_minor(&mut roots);
        h.collect_minor(&mut roots);
        assert_eq!(h.stats().promotions, 2);
        // now allocate a fresh young object referenced only from the promoted
        // outer list, and force a minor collection
        let fresh = h.alloc(Obj::List(vec![Value::Int(1)]));
        match h.get_mut(outer) {
            Obj::List(v) => v.push(Value::List(fresh)),
            _ => panic!(),
        }
        h.write_barrier(outer);
        let mut roots = vec![&mut regs];
        h.collect_minor(&mut roots);
        // fresh must have survived via the remembered old->young reference
        match h.get(fresh) {
            Obj::List(v) => assert_eq!(v[0], Value::Int(1)),
            _ => panic!("fresh young object was collected despite old->young ref"),
        }
    }

    #[test]
    fn cycles_are_reclaimed_when_unreachable() {
        let mut h = Heap::new();
        let a = h.alloc(Obj::List(vec![]));
        let b = h.alloc(Obj::List(vec![Value::List(a)]));
        match h.get_mut(a) {
            Obj::List(v) => v.push(Value::List(b)), // cycle a <-> b
            _ => panic!(),
        }
        let mut regs: Vec<Value> = vec![]; // nothing roots the cycle
        let mut roots = vec![&mut regs];
        h.collect_major(&mut roots);
        assert_eq!(h.stats().objects_freed, 2);
        assert_eq!(h.stats().live_slots, 0);
    }

    #[test]
    fn cyclic_live_objects_survive() {
        let mut h = Heap::new();
        let a = h.alloc(Obj::List(vec![]));
        let b = h.alloc(Obj::List(vec![Value::List(a)]));
        match h.get_mut(a) {
            Obj::List(v) => v.push(Value::List(b)),
            _ => panic!(),
        }
        let mut regs: Vec<Value> = vec![Value::List(a)];
        let mut roots = vec![&mut regs];
        h.collect_major(&mut roots);
        h.collect_major(&mut roots);
        assert_eq!(h.stats().objects_freed, 0);
        assert_eq!(h.stats().live_slots, 2);
    }

    #[test]
    fn repeated_allocation_is_bounded() {
        let mut h = Heap::new();
        h.young_threshold = 8;
        let mut regs: Vec<Value> = vec![Value::List(h.alloc(Obj::List(vec![])))];
        for i in 0..10_000 {
            let r = h.alloc(Obj::List(vec![Value::Int(i)]));
            let mut roots = vec![&mut regs];
            if h.needs_minor() {
                h.collect_minor(&mut roots);
            }
            // the temporary list is immediately dead
            let _ = r;
        }
        assert!(h.stats().minor_collections > 0);
        assert!(h.stats().objects_freed > 0);
        // Without a trailing collection, up to `threshold - 1` uncollected
        // temporaries remain counted; one final collection with correct
        // roots must leave exactly the rooted list (promoted to old gen).
        let mut roots = vec![&mut regs];
        h.collect_minor(&mut roots);
        let stats = h.stats();
        assert_eq!(stats.live_slots, 1, "live: {}", stats.live_slots);
        assert!(stats.live_bytes > 0);
        assert!(stats.promotions > 0, "rooted list should be promoted");
    }

    #[test]
    fn stress_mode_collects_every_allocation() {
        let mut h = Heap::new();
        h.stress = true;
        let mut regs: Vec<Value> = vec![];
        let mut roots = vec![&mut regs];
        h.maybe_collect(&mut roots);
        let _ = h.alloc(Obj::List(vec![]));
        let mut roots = vec![&mut regs];
        h.maybe_collect(&mut roots);
        assert!(h.stats().minor_collections >= 2);
    }
}

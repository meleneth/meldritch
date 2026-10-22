use meldritch2::{compiler::compile_at_rate, lua};

fn authored_patterns() -> meldritch2::model::Graph {
    lua::read(
        r#"
local i=meldritch.instrument("Arp contracts")
local p=meldritch.musical.project(i,{id="arp_test",bpm=120,max_steps=64,steps_per_beat=4})
p:track {id="Notes",kind="melodic",instrument="test"}
local notes=meldritch.arp.note_set({60,64,67})
for _,mode in ipairs({"up","down","up_down","played"}) do
  local pattern=p:pattern(mode)
  meldritch.arp.new {mode=mode,octaves=mode=="up" and 2 or 1,gate=.6}:render(pattern,"Notes",8,notes)
end
local changing=meldritch.arp.from_pattern({
  {step=1,notes={60,64}}, {step=3,notes={67,71}},
},4)
meldritch.arp.new {mode="up",restart_on_change=true}:render(p:pattern("changing"),"Notes",4,changing)
local first=meldritch.arp.new {mode="random",seed=73}
local second=meldritch.arp.new {mode="random",seed=73}
first:render(p:pattern("random_a"),"Notes",12,notes)
second:render(p:pattern("random_b"),"Notes",12,notes)
local free=meldritch.arp.new {mode="up",phase="free"}
free:render(p:pattern("free_a"),"Notes",2,notes)
free:render(p:pattern("free_b"),"Notes",2,notes)
local restart=meldritch.arp.new {mode="up",phase="restart"}
restart:render(p:pattern("restart_a"),"Notes",2,notes)
restart:render(p:pattern("restart_b"),"Notes",2,notes)
meldritch.arp.new {mode="played",clock_division=2,gate=.45,
  velocity_shape={1,.5}}:render(p:pattern("shaped"),"Notes",8,
  meldritch.arp.note_set({{note=67,velocity=.8},{note=60,velocity=.6}}))
local silent=i:constant {value=0};i:connect(silent.out,i.output.left);i:connect(silent.out,i.output.right)
i:performance {layout="step_grid",sequencer=p.sequence,keys={}}
return i
"#,
    )
    .unwrap()
}

fn midi(frequency: f32) -> i32 {
    (69.0 + 12.0 * (f64::from(frequency) / 440.0).log2()).round() as i32
}

fn notes(graph: &meldritch2::model::Graph, name: &str) -> Vec<i32> {
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    let index = sequence
        .pattern_names()
        .iter()
        .position(|candidate| candidate == name)
        .unwrap();
    let pattern = sequence.pattern_at(index).unwrap();
    let length = usize::from(pattern.lengths[0]);
    (0..length)
        .filter(|step| pattern.velocities[0][*step] > 0.0)
        .map(|step| midi(pattern.pitches_hz[0][step]))
        .collect()
}

#[test]
fn traversal_octaves_and_note_set_changes_are_authored_as_normal_events() {
    let graph = authored_patterns();
    assert_eq!(notes(&graph, "up"), [60, 64, 67, 72, 76, 79, 60, 64]);
    assert_eq!(notes(&graph, "down"), [67, 64, 60, 67, 64, 60, 67, 64]);
    assert_eq!(notes(&graph, "up_down"), [60, 64, 67, 64, 60, 64, 67, 64]);
    assert_eq!(notes(&graph, "played"), [60, 64, 67, 60, 64, 67, 60, 64]);
    assert_eq!(notes(&graph, "changing"), [60, 64, 67, 71]);
}

#[test]
fn deterministic_randomness_and_phase_policies_are_reproducible() {
    let first = authored_patterns();
    let second = authored_patterns();
    assert_eq!(notes(&first, "random_a"), notes(&first, "random_b"));
    assert_eq!(notes(&first, "random_a"), notes(&second, "random_a"));
    assert_eq!(notes(&first, "free_a"), [60, 64]);
    assert_eq!(notes(&first, "free_b"), [67, 60]);
    assert_eq!(notes(&first, "restart_a"), notes(&first, "restart_b"));
}

#[test]
fn clock_division_gate_and_velocity_shaping_preserve_note_off_gaps() {
    let graph = authored_patterns();
    let sequence = &graph.performance.as_ref().unwrap().sequencer;
    let index = sequence
        .pattern_names()
        .iter()
        .position(|candidate| candidate == "shaped")
        .unwrap();
    let pattern = sequence.pattern_at(index).unwrap();
    let active: Vec<_> = (0..8)
        .filter(|step| pattern.velocities[0][*step] > 0.0)
        .collect();
    assert_eq!(active, [0, 2, 4, 6]);
    assert!((pattern.velocities[0][0] - 0.8).abs() < 1e-6);
    assert!((pattern.velocities[0][2] - 0.3).abs() < 1e-6);
    for step in active {
        assert!((pattern.gates[0][step] - 0.45).abs() < 1e-6);
        assert!(pattern.gates[0][step] < 1.0);
    }
}

#[test]
fn rendered_arp_gate_publishes_note_off_before_the_next_retrigger() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut program = lua::read_program_with_root(
        r#"
local i=meldritch.instrument("Arp note off")
local p=meldritch.musical.project(i,{id="arp_off",bpm=120,max_steps=8,steps_per_beat=4})
p:track {id="Notes",kind="melodic",instrument="test"}
meldritch.arp.new {clock_division=8,gate=.45}:render(p:pattern("Only"),"Notes",8,{60})
p.sequence:on("note_on",function(_) end)
p.sequence:on("note_off",function(_) end)
local silent=i:constant {value=0};i:connect(silent.out,i.output.left);i:connect(silent.out,i.output.right)
return i
"#,
        root,
    )
    .unwrap();
    let mut machine = compile_at_rate(program.graph(), 8.0).unwrap();
    machine.process_stereo(&mut [0.0; 3], &mut [0.0; 3]);
    let diagnostics = machine.sequencer_controls()[0].event_source().diagnostics();
    assert_eq!(
        diagnostics.publications, 2,
        "expected one note-on and one note-off"
    );
    assert_eq!(program.events_mut().dispatch(), 2);
}

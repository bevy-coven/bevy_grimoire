// The doorway is a volume at the gate. It tells the referee the hero came through, and only when the gate is open:
// walking over the sill of a shut gate is walking past a wall.
fn on_trigger(other) {
    let gate = find("gate");
    if other == "hero" && gate != () && gate.Door.open {
        emit("through_gate");
    }
}

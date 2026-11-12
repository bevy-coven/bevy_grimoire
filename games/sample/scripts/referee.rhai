// The win condition: the hero has come through the open gate, and the crown is gone from the floor.
fn on_start() {
    this.state.through = false;
    this.state.won = false;
}

fn on_event(name, data) {
    if name == "through_gate" {
        this.state.through = true;
    }
}

fn on_update(dt) {
    if this.state.won || !this.state.through {
        return;
    }
    if find("crown") == () {
        this.state.won = true;
        emit("win");
    }
}

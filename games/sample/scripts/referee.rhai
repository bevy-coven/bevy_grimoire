// The win condition: the hero has come through the open gate and taken the crown that lay behind it.
//
// An event a hook emits during the frame's own hooks is delivered to every script that frame, and only an event emitted from
// an `on_event` reaches a listener outside the scripts. So the referee asks itself once a frame (`check_win`) and says `win`
// from the answer: the one place a script's word is heard by the game.
fn on_update(dt) {
    emit("check_win");
}

fn on_event(name, data) {
    if name == "through_gate" {
        this.state.through = true;
    }
    if name == "check_win" && (this.state.through ?? false) && !(this.state.won ?? false) {
        if find("crown") == () {
            this.state.won = true;
            emit("win");
        }
    }
}

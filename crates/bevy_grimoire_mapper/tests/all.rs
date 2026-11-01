//! **Every integration test of the editor, in one test binary.** (owner's session, 2026-10-29)
//!
//! Each `tests/*.rs` used to be its own binary, and each of the ~150 linked a whole Bevy app: the
//! loop's push gate (`cargo test --workspace`) spent most of an hour linking them on a 10-core Mac,
//! and timed out twice in one pass (`bevy_emerge_loop`, pass 128). As modules of one binary they are
//! linked once and share Bevy's monomorphised code. `Cargo.toml` sets `autotests = false`, so a file
//! under `tests/` is compiled only when it is a module here; `every_test_file_is_a_module_here`
//! fails when one is not.
//!
//! **Adding a test file:** add `mod <file_stem>;` below, in order, and write `use crate::fixtures;`
//! (not `mod fixtures;`) in the file. Run one file's tests with
//! `cargo test -p bevy_grimoire_mapper --test editor <file_stem>::`.
//!
//! **Tests now share a process**, with tests from other files running beside them: a scratch path
//! must be unique to its test (its name and `std::process::id()`), never a fixed name.

mod fixtures;

mod a_broken_stack_still_opens_framed;
mod a_capture_sequence_takes_one_frame_at_a_time;
mod a_change_says_what_it_broke;
mod a_change_whose_log_line_cannot_be_written_is_taken_back;
mod a_chip_click_writes_the_library_once;
mod a_click_outside_the_viewport_places_nothing;
mod a_composed_proposal_draws_its_members;
mod a_composition_can_be_renamed;
mod a_damaged_oplog_line_is_named_and_the_rest_still_reads;
mod an_oplog_line_of_another_version_is_refused;
mod a_dead_end_names_its_closest_piece;
mod a_destructible_file_of_another_version_is_refused;
mod a_destructible_file_that_does_not_read_is_named;
mod a_door_answer_says_whether_it_waits;
mod a_dropped_image_becomes_a_reference;
mod a_dressed_camera_leaves_the_world_on_the_surface;
mod a_file_says_what_the_game_made_of_its_edit;
mod a_file_that_does_not_read_is_never_overwritten;
mod a_fill_tells_the_truth;
mod a_forest_people_or_scripts_file_of_another_version_is_refused;
mod a_full_redraw_leaves_the_standing_pieces_standing;
mod a_game_data_file_that_does_not_read_is_named;
mod a_height_map_becomes_terrain;
mod forest_place;
mod forest_season;
mod forest_stroke_is_an_operation;
mod forest_view;
mod a_hint_shares_the_receipts_slot;
mod a_history_jump_is_one_undo_group;
mod a_key_set_in_both_settings_layers_takes_the_projects;
mod a_kit_edited_on_disk_reaches_the_screen;
mod a_kit_s_dead_ends_are_shown;
mod a_kit_save_rereads_only_that_kit;
mod a_label_and_its_value_are_two_things;
mod a_label_set_on_effects_settles_as_the_kind_does;
mod a_vlm_label_is_suggested_then_applied;
mod a_library_change_does_not_block_another_pieces_undo;
mod a_made_map_can_be_taken_away_by_undo;
mod a_map_can_be_renamed_through_the_registry;
mod a_map_edited_on_disk_reaches_the_screen;
mod a_meshes_undo_takes_back_the_agents_newest_label;
mod a_note_box_can_say_auto;
mod a_note_on_picked_pieces_is_saved;
mod a_palette_click_arms_the_piece;
mod a_person_removes_a_box_and_undoes_it;
mod a_person_removes_a_piece_and_undoes_it;
mod a_person_shift_clicks_two_pieces_into_one_note;
mod a_person_snips_an_area_of_the_editor;
mod a_persons_turn_is_one_undo_step;
mod a_piece_lands_where_the_pointer_is;
mod a_placement_in_a_doorway_says_so;
mod a_placement_in_reserved_space_says_so;
mod a_preview_draws_at_the_level_the_piece_lands_at;
mod a_project_file_created_while_open_is_read;
mod a_projects_repetition_rules_decide_what_is_structure;
mod a_proposal_is_drawn_before_it_is_kept;
mod a_proposal_offers_options;
mod a_proposal_says_how_varied_it_is;
mod a_question_is_asked_in_a_gooey_dialog;
mod a_receipt_does_not_move_the_panel;
mod a_rectangle_frames_what_stands_in_it;
mod a_reference_image_that_does_not_read_is_named;
mod a_refusal_names_the_kit_that_owns_the_piece;
mod a_refusal_reaches_the_author;
mod a_refused_checks_file_keeps_the_last_good_checks;
mod a_refused_notes_file_keeps_the_last_good_notes;
mod a_refused_operation_says_the_same_thing_on_every_surface;
mod a_refused_redo_keeps_its_entry;
mod a_refused_terrain_file_keeps_the_last_good_drawing;
mod a_refused_tree_file_keeps_the_last_good_tree;
mod a_refused_undo_keeps_its_entry;
mod a_rename_carries_the_rules_example_with_it;
mod a_rename_is_all_or_nothing;
mod a_rename_whose_old_file_cannot_go_changes_nothing;
mod a_rigs_edit_says_what_became_of_it;
mod a_rule_can_be_changed;
mod a_run_lays_a_line_of_pieces;
mod a_sequence_started_behind_schedule_still_applies_its_operation;
mod a_sequence_starts_before_the_action;
mod a_sequence_takes_the_agents_checks;
mod a_sequence_that_opens_a_door_crosses_to_it;
mod a_sequence_turn_is_on_the_receipt;
mod a_snip_lists_the_stamped_rows_it_covers;
mod a_script_edited_on_disk_takes_effect;
mod a_stale_entry_is_dropped_on_its_second_refusal;
mod a_stale_snip_is_swept_on_open;
mod a_stamp_turns_by_the_unit_its_schema_names;
mod a_terrain_file_that_does_not_fill_its_shape_is_refused;
mod a_terrain_water_or_volume_file_of_another_version_is_refused;
mod a_torn_oplog_line_does_not_swallow_the_next_entry;
mod a_trigger_binds_to_a_notes_area;
mod a_two_way_mount_is_seated_by_every_door;
mod a_two_way_mount_seats_on_either;
mod a_weight_names_the_pieces_it_tunes;
mod after_edit_checks_say_what_they_found;
mod after_edit_waits_for_the_picture;
mod an_agent_can_define_a_composition;
mod an_agent_can_set_a_composition_envelope;
mod an_agent_can_stamp_a_composition;
mod an_agent_can_drive_the_editor;
mod an_agent_can_enter_the_map_it_made;
mod an_agent_can_fill;
mod an_agent_can_generate_a_layout;
mod an_agent_can_import_a_mesh;
mod an_agent_can_make_a_map;
mod an_agent_can_place_a_piece;
mod an_agent_can_search_the_operations;
mod an_agent_can_show_its_plan;
mod an_agent_can_take_its_own_change_back;
mod an_agent_change_is_on_the_receipt;
mod an_agent_presents_its_result;
mod an_agent_turn_in_a_sequence_stands_on_the_screen;
mod an_agents_edit_owes_checks_a_persons_does_not;
mod an_agents_steps_are_on_screen;
mod an_agents_turn_is_one_undo_step;
mod an_agents_undo_can_be_redone_by_the_person;
mod an_f32_in_the_log_still_undoes;
mod an_imported_piece_names_the_standards_it_breaks;
mod an_undo_across_a_rename_names_the_rename;
mod an_unsaved_edit_survives_the_watcher;
mod asset_spike;
mod b027_the_pin_reason_names_its_piece;
mod before_picture_is_the_last_after;
mod brush_arm_is_an_operation;
mod carved_ground_is_drawn;
mod census_is_the_one_counter;
mod check_view_asks_the_checks_on_demand;
mod chrome_census;
mod closeup_frames_the_edit;
mod compose_is_read_only;
mod composition_arm_is_an_operation;
mod composition_rename_is_an_operation;
mod crowd_heat;
mod crowd_preview_is_an_operation;
mod crowd_preview_measures;
mod data_set_is_an_operation;
mod decision_labels_mount_front;
mod destructible_marks_bake_once;
mod door_open_is_an_operation;
mod e024_an_agent_can_pin_a_placement;
mod e026_grimoire_state_reports_each_pin;
mod grimoire_pieces_lists_the_library;
mod grimoire_state_names_modules_that_do_not_nest;
mod grimoire_state_names_the_library;
mod grimoire_state_names_visible_repeats;
mod grimoire_state_reports_broken_relations;
mod grimoire_state_reports_standards_breaches;
mod grimoire_state_says_whether_the_kit_loops_and_opens_into_a_room;
mod environment_applies;
mod every_action_is_a_command;
mod every_key_has_a_home;
mod editor_skin;
mod editor_ui_plugin;
mod every_list_follows_its_selection;
mod every_refusal_is_in_a_problem;
mod every_resource_says_what_a_door_does_to_it;
mod family_carnage_settings;
mod dialogue_preview;
mod dialogue_runtime;
mod people;
mod people_ties;
mod person_talks;
mod family_dialogue;
mod family_gooey_skin;
mod family_host;
mod family_lights_settings;
mod flora_family;
mod family_set_is_an_operation;
mod family_society;
mod placement_family;
mod family_settings_reload;
mod family_settings_verdicts;
mod family_settings_at_launch;
mod family_settings_typed_verdicts;
mod font_fallback;
mod frame_the_view;
mod headless;
mod home_frames_the_view;
mod inspector_names_broken_relations;
mod inspector_names_the_worst_repeat;
mod label_set_is_an_operation;
mod leaf;
mod light_map;
mod light_overlay;
mod light_pool;
mod light_view;
mod live_writes_each_change_through;
mod no_observer_hides_a_resource_in_a_bundle;
mod no_system_writes_every_frame;
mod note_auto;
mod note_claims;
mod note_cites_reference;
mod note_fence;
mod note_undo_work;
mod note_work_reads_the_whole_log;
mod notes_are_operations;
mod notes_end_to_end;
mod notes_history_and_send;
mod notes_panel_lists_every_note;
mod notes_panel_shows_the_auto_flag;
mod one_prompt_one_vocabulary;
mod one_undo_takes_back_the_newest_change;
mod operations_have_two_front_ends;
mod out_of_area_compares_before_and_after;
mod pack_set_excluded_is_an_operation;
mod pieces_stand_on_sculpted_ground;
mod piece_remove_is_an_operation;
mod piece_select_is_an_operation;
mod piece_set_mount_is_an_operation;
mod play_starts_and_stops_the_game;
mod play_stop_ends_the_whole_game;
mod placement_remove;
mod placement_set_lift_is_an_operation;
mod placement_set_pose_is_an_operation;
mod placement_set_tip_is_an_operation;
mod placement_set_yaw;
mod placement_undo_without_id;
mod project_clone_shares;
mod references;
mod recent_projects;
mod relations_read_a_turned_piece_in_degrees;
mod relations_solve_mends_the_open_map;
mod reload_contract;
mod repetition_vary_turns_the_open_maps_repeats;
mod roads_and_fields_do_not_z_fight;
mod rows_become_a_run_through_an_operation;
mod runs_are_edited_through_operations;
mod send_starts_the_notes_agent;
mod stamp_remove;
mod terrain_preset_is_an_operation;
mod terrain_stroke_is_an_operation;
mod terrain_view_draws_water;
mod the_arm_crosses_the_door;
mod the_arm_survives_the_door_change;
mod the_door_switch_crosses;
mod the_editor_and_the_game_agree;
mod the_enclosure_wish_is_the_maps_to_set;
mod the_games_per_file_verdicts_reach_grimoire_state;
mod the_games_verdict_reaches_grimoire_state;
mod the_ghost_is_the_contract;
mod the_interface_does_not_write_the_ground_back_over_the_world;
mod the_journal_draws_the_agents_steps;
mod the_layout_score_counts_crossings;
mod the_library_seed_is_a_selection;
mod the_map_opens_framed;
mod the_meshes_pane_names_modules_that_do_not_nest;
mod the_meshes_pane_names_the_standards_a_piece_breaks;
mod the_meshes_pane_says_how_sure_the_labels_are;
mod the_next_undo_is_named;
mod the_note_box_is_on_screen_while_typed;
mod the_plan_panel_lists_the_agents_steps;
mod the_pointer_is_read_before_anything_acts_on_it;
mod the_refusal_is_on_screen;
mod the_run_receipt_names_the_triggers_that_fired;
mod the_scripts_panel_lists_the_maps_triggers;
mod the_stage_labels_are_on_screen;
mod the_stage_labels_do_not_collide;
mod the_stage_labels_live_in_one_space;
mod the_state_carries_a_frame_clock;
mod the_surface_and_the_camera_agree;
mod the_sweep_is_finished;
mod the_toast_is_on_screen;
mod the_viewport_keeps_a_usable_width_in_a_narrow_window;
mod the_way_back_actually_goes_back;
mod the_whole_map_is_inside_the_cameras_depth;
mod tree_editor;
mod triggers;
mod two_writers_never_share_a_sequence_number;
mod volume_edit_is_an_operation;
mod volume_meshes_without_cracks;
mod volume_snow;
mod volume_wears_its_cover;
mod water_place_is_an_operation;
mod water_run_settles_a_pond;
mod world_gizmos_stay_in_the_world;

/// A file under `tests/` that is not a module here is never compiled, so its tests would silently
/// stop running.
#[test]
fn every_test_file_is_a_module_here() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let listed = include_str!("all.rs");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("tests/ is readable") {
        let path = entry.expect("a directory entry").path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else { continue };
        if path.extension().is_some_and(|e| e == "rs")
            && stem != "all"
            && !listed.lines().any(|line| line == format!("mod {stem};"))
        {
            missing.push(stem.to_owned());
        }
    }
    missing.sort();
    assert!(
        missing.is_empty(),
        "add `mod <name>;` to crates/bevy_grimoire_mapper/tests/all.rs (and `use crate::fixtures;` in the file) for: {missing:?}"
    );
}

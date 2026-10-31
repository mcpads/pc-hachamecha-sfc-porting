MAX_PAN = 128

N_NOTE_OCTAVES = 8

N_PLAY_NOTE_INSTRUCTIONS = N_NOTE_OCTAVES * 12 * 2

FIRST_PLAY_NOTE_INSTRUCTION = 64
FIRST_NO_ARGUMENT_INSTRUCTION_OPCODE = 56

DISABLE_CHANNEL_BYTECODE = FIRST_PLAY_NOTE_INSTRUCTION - 1

.ftdef INSTRUCTIONS_WITH_ARGUMENTS

    bc_ReservedForCustomUse

    bc__miscellaneous

    bc__portamento_down
    bc__portamento_up

    bc__portamento_calc

    bc__portamento_pitch_down
    bc__portamento_pitch_up

    bc__portamento_pitch_calc

    bc__set_vibrato

    bc__set_vibrato_with_delay

    bc__set_vibrato_depth_and_play_note

    bc__play_pitch

    bc__play_noise

    bc__wait

    bc__rest

    bc__set_instrument

    bc__set_instrument_and_adsr_or_gain

    bc__set_adsr

    bc__set_gain

    bc__set_temp_gain

    bc__set_temp_gain_and_wait

    bc__set_temp_gain_and_rest

    bc__reuse_temp_gain_and_wait

    bc__reuse_temp_gain_and_rest

    bc__set_early_release

    bc__set_early_release_no_minimum

    bc__set_transpose

    bc__adjust_transpose

    bc__set_detune_i16

    bc__set_detune_p8

    bc__set_detune_n8

    bc__adjust_pan

    bc__set_pan

    bc__set_pan_and_volume

    bc__adjust_volume

    bc__set_volume

    bc__set_channel_or_echo_invert

    bc__volume_slide_up

    bc__volume_slide_down

    bc__tremolo

    bc__pan_slide_up

    bc__pan_slide_down

    bc__panbrello

    bc__start_loop

    bc__skip_last_loop_u8

    bc__skip_last_loop_u16be

    bc__call_subroutine_and_disable_vibrato

    bc__call_subroutine

    bc__goto_relative

    bc__set_echo_volume

    bc__set_stereo_echo_volume

    bc__adjust_echo_volume

    bc__adjust_stereo_echo_volume

    bc__set_fir_filter

    bc__set_or_adjust_global_i8

    bc__adjust_global_i8_limit
.endftdef

.ftdef NO_ARGUMENT_INSTRUCTIONS

    bc__end_loop

    bc__return_from_subroutine_and_disable_vibrato

    bc__return_from_subroutine

    bc__enable_echo

    bc__disable_echo

    bc__reuse_temp_gain

    bc__keyon_next_note

    bc__disable_channel
.endftdef

    DISABLE_NOISE_OPCODE = %000000_00

    DISABLE_PMOD_OPCODE = %000001_00

    ENABLE_PMOD_OPCODE = %000000_10

    SET_SONG_TICK_CLOCK_OPCODE = %000000_01

    SET_ECHO_DELAY_OPCODE = %000000_11

ENABLE_PMOD_MASK = %00111110

GLOBAL_I8_EFB_INDEX = 8
GLOBAL_I8_MAIN_VOLUME_INDEX = 9

MAX_GLOBAL_I8_INDEX = GLOBAL_I8_MAIN_VOLUME_INDEX
MAX_SET_OR_ADJUST_GLOBAL_I8_PARAM = (MAX_GLOBAL_I8_INDEX << 1) | 1

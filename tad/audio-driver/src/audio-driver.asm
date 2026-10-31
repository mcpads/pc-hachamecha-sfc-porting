.include "registers.inc"
.include "data-formats.inc"
.include "io-commands.inc"
.include "common-memmap.inc"
.include "bytecode.inc"

.codebank CODE_ADDR..CODE_END_ADDR

.varbank CAD COMMON_DATA_ADDR..COMMON_DATA_ADDR+COMMON_DATA_HEADER_SIZE+1
.vars CAD
    commonDataHeader : CommonDataHeader

    brrDirectory : u8
.endvars
.assert brrDirectory - commonDataHeader == COMMON_DATA_HEADER_SIZE

EDL_SLEEP_MS = 250

EDL_SLEEP_TIMER_1 = EDL_SLEEP_MS
EDL_SLEEP_COUNT   = TIMER_1_HZ / 1000

I8_MIN = -128
I8_MAX = 127

FIRST_MUSIC_CHANNEL = 0
LAST_MUSIC_CHANNEL = N_MUSIC_CHANNELS - 1
FIRST_SFX_CHANNEL = LAST_MUSIC_CHANNEL + 1
LAST_SFX_CHANNEL = N_CHANNELS - 1

VOL_PAN_EFFECT_SLIDE_UP      = $80
VOL_PAN_EFFECT_SLIDE_DOWN    = $81
VOL_PAN_EFFECT_TRIANGLE_UP   = $40
VOL_PAN_EFFECT_TRIANGLE_DOWN = $41

.vars zeropage

    streamWritePointer : ptr

    zpTmpWord : u16
        zpTmp  = zpTmpWord
        zpTmp2 = zpTmpWord + 1
    zpTmp3 : u8

    songTickCounter : u16

    io_musicChannelsMask : u8

    subroutineTable_l : ptr
    subroutineTable_h : ptr

    lagDetector : u8
        LD_MUSIC_LAG_BIT = 0
        LD_SFX_LAG_BIT = 1

        LD_MUSIC_TEST_BIT = 6
        LD_SFX_TEST_BIT = 7

    musicSfxChannelMask : u8

    sfxMusicEchoNoiseMask : u8

    activeSoundEffects : u8

    prevActiveSoundEffects : u8

    sfx_interruptibleFlags : u8

    sfx_remainingTicks : [u16 : N_SFX_CHANNELS]
        sfx_remainingTicks__ELEMENT_SIZE = 2

    sfx_oneChannelSfxId : [u8 : N_SFX_CHANNELS]
        sfx_oneChannelSfxId__ELEMENT_SIZE = 1

    voiceChannelsDirty_music : u8
    voiceChannelsDirty_sfx   : u8

    keyOffShadow_music : u8
    keyOffShadow_sfx : u8

    keyOnShadow_music : u8
    keyOnShadow_sfx : u8

    pmonShadow : u8

    eonShadow_music : u8
    eonShadow_sfx : u8

    keyOnMask_music : u8
    keyOnMask_sfx : u8

    instructionPtr : ptr

    voiceChannelsDirty_tmp  : u8

    globalVolume_tmp : u8

    channelSoA_countdownTimer_l : [u8 : N_CHANNELS]

    channelSoA_keyoffMsbFlag : [u8 : N_CHANNELS]

        channelSoA_virtualChannels_vol_l       : [u8 : N_CHANNELS]
        channelSoA_virtualChannels_vol_r       : [u8 : N_CHANNELS]
        channelSoA_virtualChannels_pitch_l     : [u8 : N_CHANNELS]
        channelSoA_virtualChannels_pitch_h     : [u8 : N_CHANNELS]
        channelSoA_virtualChannels_srcn        : [u8 : N_CHANNELS]
        channelSoA_virtualChannels_adsr1       : [u8 : N_CHANNELS]

        channelSoA_virtualChannels_adsr2OrGain : [u8 : N_CHANNELS]

        channelSoA_virtualChannels_tempGain : [u8 : N_CHANNELS]

    channelSoA_instructionPtr_h : [u8 : N_CHANNELS]

    channelSoA_loopStackPointer : [u8 : N_CHANNELS]

    channelSoA_portamento_direction : [i8 : N_CHANNELS]

    channelSoA_vibrato_pitchOffsetPerTick : [u8 : N_CHANNELS]

    channelSoA_vibrato_direction : [u8 : N_CHANNELS]

    volShadowDirty_tmp : u8

    volShadowDirty_music : u8

    volShadowDirty_sfx : u8

    noiseLock : u8

    lastSfxPlayNoiseChannel : u8

    noiseFreq_music : u8
    noiseFreq_sfx : [u8: N_SFX_CHANNELS]

    nonShadow_music : u8
    nonShadow_sfx : u8

__EndZeropageClearAddr = nonShadow_sfx + 1

    maxEdl : u8

    songGlobalsDirty : u8
        SONG_GLOBALS_DIRTY__FIR_FILTER_BIT = 7
        SONG_GLOBALS_DIRTY__CLEAR_FIR_BIT = 0
        SONG_GLOBALS_DIRTY__ECHO_VOLUME_BIT = 6

        SONG_GLOBALS_DIRTY__EFB_OR_MVOL_BIT = 5
        SONG_GLOBALS_DIRTY__EDL_BIT = 4

    songGlobals : GlobalSongSettings

    commonData : CommonDataPointers
.endvars

.vars firstpage

    pendingNon_music : u8
    pendingNon_sfx : u8

    sfxMutedChannels : u8

    channelSoA_vibrato_delay : [u8 : N_CHANNELS]

    channelSoA_vibrato_tickCounter : [u8 : N_CHANNELS]

    channelSoA_vibrato_halfWavelength : [u8 : N_CHANNELS]

    channelSoA_vibrato_tickCounterStart : [u8 : N_CHANNELS]

    channelSoA_detune_l : [u8 : N_CHANNELS]
    channelSoA_detune_h : [u8 : N_CHANNELS]

    channelSoA_invertFlags : [u8 : N_CHANNELS]

    channelSoA_volume : [u8 : N_CHANNELS]

    channelSoA_subVolume : [u8 : N_CHANNELS]

    channelSoA_volEffect_direction : [u8 : N_CHANNELS]

    channelSoA_volEffect_counter : [u8 : N_CHANNELS]

    channelSoA_volEffect_offset_l : [u8 : N_CHANNELS]
    channelSoA_volEffect_offset_h : [u8 : N_CHANNELS]

    channelSoA_volEffect_halfWavelength : [u8 : N_CHANNELS]

    channelSoA_pan : [u8 : N_CHANNELS]

    channelSoA_subPan : [u8 : N_CHANNELS]
    channelSoA_panEffect_direction : [u8 : N_CHANNELS]
    channelSoA_panEffect_counter : [u8 : N_CHANNELS]
    channelSoA_panEffect_offset_l : [u8 : N_CHANNELS]
    channelSoA_panEffect_offset_h : [u8 : N_CHANNELS]
    channelSoA_panEffect_halfWavelength : [u8 : N_CHANNELS]

    channelSoA_earlyRelease_cmp : [u8 : N_CHANNELS]

    previousCommand : u8
.endvars

.vars lastvarpage

    channelSoA_countdownTimer_h : [u8 : N_CHANNELS]

    channelSoA_transpose : [u8 : N_CHANNELS]

    channelSoA_earlyRelease_minTicks : [u8 : N_CHANNELS]
    channelSoA_earlyRelease_gain : [u8 : N_CHANNELS]

    channelSoA_instructionPtr_l : [u8 : N_CHANNELS]

    channelSoA_portamento_speed : [u8 : N_CHANNELS]

    channelSoA_portamento_target_l : [u8 : N_CHANNELS]
    channelSoA_portamento_target_h : [u8 : N_CHANNELS]

    channelSoA_stackPointer : [u8 : N_CHANNELS]

    channelSoA_prevTempGain : [u8 : N_CHANNELS]

    channelSoA_ticksAfterNote : [u8 : N_CHANNELS]

    __bcStack : [u8 : BC_CHANNEL_STACK_SIZE * N_CHANNELS]
        bcStack = __bcStack - BC_CHANNEL_STACK_OFFSET
.endvars

.proc main

    .assert PC == CODE_ADDR

    mov X, #$ff
    mov SP, X
    clrp
.p0

    mov A, #DSP_FLG
    mov Y, #DSP_FLG__SOFT_RESET | DSP_FLG__MUTE_ALL | DSP_FLG__ECHO_DISABLE
    movw DSPADDR, YA

    mov A, #DSP_KOFF
    mov Y, #$ff
    movw DSPADDR, YA

    .assert _songHeader > __EndZeropageClearAddr
    .assert _activeMusicChannels > __EndZeropageClearAddr
    .assert _songHeader + SONG_HEADER_SIZE < SHARED_ZEROPAGE_ADDR

    .assert offsetof(SongHeader, nSubroutines) + 1 == SONG_HEADER_SIZE

    _songHeader = songGlobals - offsetof(SongHeader, globals)
    _activeMusicChannels = _songHeader + offsetof(SongHeader, activeMusicChannels)
    _songHeader_tickTimer = _songHeader + offsetof(SongHeader, tickTimer)
    _songHeader_nSubroutines = _songHeader + offsetof(SongHeader, nSubroutines)

    .assert offsetof(SongHeader, activeMusicChannels) == 0
    mov Y, #offsetof(SongHeader, activeMusicChannels)
    mov A, [songPtr] + Y
    mov _activeMusicChannels, A

    .assert offsetof(SongHeader, globals) == 1
    mov Y, #offsetof(SongHeader, globals)
    EchoCopyLoop:
        mov A, _activeMusicChannels
        beq SkipHeaderByte
            mov A, [songPtr] + Y
        SkipHeaderByte:

        mov _songHeader + Y, A

        inc Y
        cmp Y, #SONG_HEADER_SIZE
        bcc EchoCopyLoop

    mov A, _activeMusicChannels
    bne NoSetMainVolume
        mov songGlobals.mainVolume, #127
    NoSetMainVolume:

    mov A, songGlobals.edl
    xcn A
    and A, #$0f
    mov maxEdl, A

    xcn A
    lsr A

    eor A, #$ff
    inc A

    bne     EdlNotZero

        dec A
    EdlNotZero:

    mov DSPADDR, #DSP_ESA
    mov DSPDATA, A

    push A

    mov A, songGlobals.edl
    and A, #$0f
    cmp A, maxEdl
    bcc EdlOk
        mov A, maxEdl
    EdlOk:
    mov songGlobals.edl, A

    .assert DSP_EDL == DSP_ESA | (1 << 4)
    set1 DSPADDR, 4
    mov DSPDATA, A

    mov T1TARGET, #EDL_SLEEP_TIMER_1

    mov CONTROL, #0
    mov CONTROL, #CONTROL__ENABLE_TIMER_1

    mov Y, #0
    mov A, #DSP_EON
    movw DSPADDR, YA

    .assert lobyte(brrDirectory) == 0
    mov A, #DSP_DIR
    mov Y, #hibyte(brrDirectory)
    movw DSPADDR, YA

    mov A, #0
    mov X, #__EndZeropageClearAddr
    ZeropageClearLoop:
        dec X
        mov (X), A
        bne ZeropageClearLoop

    mov pendingNon_sfx, A
    mov pendingNon_music, A
    mov sfxMutedChannels, A

    bbc loaderDataType, LoaderDataType__RESET_GLOBAL_VOLUMES_BIT, NoResetGlobalVolume

        mov globalVolume_music, A
        mov globalVolume_sfx, A
    NoResetGlobalVolume:

    .assert N_MUSIC_CHANNELS == 8

    mov X, #N_MUSIC_CHANNELS - 1
    mov Y, #SONG_HEADER_SIZE
    SetupChannelsLoop:
        asl _activeMusicChannels
        bcc SkipChannel

            mov A, [songPtr] + Y
            clrc
            adc A, songPtr
            mov channelSoA_instructionPtr_l + X, A
            inc Y

            mov A, [songPtr] + Y
            adc A, songPtr+1

            inc Y

            bcc ChannelOk

                mov A, #0
        ChannelOk:
            mov channelSoA_instructionPtr_h + X, A
    SkipChannel:

        push Y

        mov A, #MAX_PAN / 2
        call __reset_channel

        pop Y

        dec X
        bpl SetupChannelsLoop

    mov A, Y
    mov Y, #0
    addw YA, songPtr
    movw subroutineTable_l, YA

    mov A, _songHeader_nSubroutines
    mov Y, #0
    addw YA, subroutineTable_l
    movw subroutineTable_h, YA

    mov T0TARGET, _songHeader_tickTimer

    mov X, #COMMON_DATA_POINTERS_SIZE - 1
    CopyCommonDataLoop:
        mov A, commonDataHeader.pointers + X
        mov commonData + X, A
        dec X
        bpl CopyCommonDataLoop

    bbs loaderDataType, LoaderDataType__SURROUND_FLAG_BIT, IsSurround
        .assert INVERT_FLAGS__MONO & %1100_0001 == 0
        mov A, songGlobals.echoInvertFlags
        and A, #INVERT_FLAGS__MONO
        beq NoMonoInvert
            mov A, #$ff
        NoMonoInvert:
        mov songGlobals.echoInvertFlags, A
    IsSurround:

    mov A, #$ff
    mov songGlobalsDirty, A
    mov io_musicChannelsMask, A
    mov musicSfxChannelMask, A
    mov volShadowDirty_music, A
    mov keyOnMask_music, A

    .assert N_SFX_CHANNELS == 2
    mov sfx_oneChannelSfxId+0, A
    mov sfx_oneChannelSfxId+1, A

    pop X

ClearEchoBufferStart:

        __clear_echo_buffer

        mov A, #EDL_SLEEP_COUNT - 1
        EchoBufferWait:
            setc
            sbc A, T1OUT
            bcs EchoBufferWait
ClearEchoBufferEnd:

    mov T1TARGET, #SFX_TICK_CLOCK

    mov A, #DSP_FLG
    mov Y, #0
    movw DSPADDR, YA

    io__init

    mov DriverIO__MODE_PORT, #DriverIO__MODE_AUDIO_DRIVER

    .assert PC == mainloop
.endproc

.proc mainloop

    mov CONTROL, #0

    bbc loaderDataType, LoaderDataType__PLAY_SONG_BIT, NoStartSong
        mov CONTROL, #CONTROL__ENABLE_TIMER_0 | CONTROL__ENABLE_TIMER_1
    NoStartSong:

    .assert T0OUT + 1 == T1OUT
    movw YA, T0OUT

    bra ProcessMusic

    Loop:
        io__process_io_ports

        mov A, T0OUT
        bne ProcessMusic
            clr1 lagDetector, LD_MUSIC_TEST_BIT
            bra EndMusic

        ProcessMusic:
            call process_music_channels

            bbc lagDetector, LD_MUSIC_TEST_BIT, NoMusicLag
                set1 lagDetector, LD_MUSIC_LAG_BIT
            NoMusicLag:
            set1 lagDetector, LD_MUSIC_TEST_BIT
        EndMusic:

        mov A, T1OUT
        bne ProcessSfx
            clr1 lagDetector, LD_SFX_TEST_BIT
            bra Loop

        ProcessSfx:
            process_sfx_channels

            bbc lagDetector, LD_SFX_TEST_BIT, NoSfxLag
                set1 lagDetector, LD_SFX_LAG_BIT
            NoSfxLag:
            set1 lagDetector, LD_SFX_TEST_BIT

            jmp Loop
.endproc

.inline __clear_echo_buffer

    .assert STA_2 - STA_1 == 3
    .assert After_STA_2 - STA_2 == 3

    mov STA_1 + 2, X
    mov STA_2 + 2, X

    mov A, #0

    OuterLoop:
        mov Y, #$80

        InnerLoop:

        STA_1:
            mov $0000 + Y, A
        STA_2:
            mov $0080 + Y, A
        After_STA_2:

            dbnz Y, InnerLoop

        inc STA_1 + 2
        inc STA_2 + 2
        bne OuterLoop
.endinline

.inline io__init

    .assert IoCommands__PAUSE == 0

    mov A, #IoCommands__PAUSE
    mov DriverIO__COMMAND_ACK_PORT, A
    mov previousCommand, A
.endinline

.inline io__process_io_ports

    mov A, DriverIO__COMMAND_PORT
    cmp A, previousCommand
    beq NoCommand

        cbne DriverIO__COMMAND_PORT, NoCommand
            push A
            mov previousCommand, A

            mov Y, DriverIO__PARAMETER_0_PORT
            call _io__call_command

            pop A
            mov DriverIO__COMMAND_ACK_PORT, A
    NoCommand:

    bbc DriverIO__SWITCH_TO_LOADER_PORT, DriverIO__SWITCH_TO_LOADER_BIT, NoStl

        bbc DriverIO__SWITCH_TO_LOADER_PORT, DriverIO__SWITCH_TO_LOADER_BIT, NoStl
            jmp LOADER_ADDR
    NoStl:
.endinline

.proc _io__call_command
    .assert CommandFunctionTable_SIZE % 2 == 0
    .assert DriverIO__COMMAND_MASK >= CommandFunctionTable_SIZE - 2

    and A, #DriverIO__COMMAND_MASK
    cmp A, #CommandFunctionTable_SIZE
    bcs Return
        mov X, A
        jmp [CommandFunctionTable+X]

Return:
    ret
.endproc

.proc cmd__stream_address
    mov streamWritePointer, Y
    mov A, DriverIO__PARAMETER_1_PORT
    mov streamWritePointer+1, A
    ret
.endproc

.proc cmd__stream_burst
    mov streamWritePointer, Y
    mov A, DriverIO__PARAMETER_1_PORT
    mov streamWritePointer+1, A
    mov X, #0
    mov Y, #0
    mov DriverIO__COMMAND_ACK_PORT, #$80
Triple:
    inc X
WaitTriple:
    cmp X, DriverIO__COMMAND_PORT
    bne WaitTriple
    mov A, DriverIO__PARAMETER_0_PORT
    mov [streamWritePointer]+Y, A
    inc Y
    mov A, DriverIO__PARAMETER_1_PORT
    mov [streamWritePointer]+Y, A
    inc Y
    mov A, DriverIO__SWITCH_TO_LOADER_PORT
    mov [streamWritePointer]+Y, A
    inc Y
    mov DriverIO__COMMAND_ACK_PORT, X
    cmp X, #LIVE_TRANSFER_BYTES / 3
    bne Triple
Release:
    mov A, previousCommand
    cbne DriverIO__COMMAND_PORT, Release
    ret
.endproc

.proc cmd__stream_play
    cmp Y, #8
    bcs Return
    mov A, Y
    and A, #1
    beq First
    jmp sfx__play_1
First:
    jmp sfx__play_sfx_0
Return:
    ret
.endproc

.proc cmd__stream_stop
    cmp Y, #2
    bcs Return
    mov A, Y
    clrc
    adc A, #FIRST_SFX_CHANNEL
    mov X, A
    mov A, #0
    mov channelSoA_instructionPtr_h + X, A
    jmp bc__disable_channel
Return:
    ret
.endproc

CommandFunctionTable:
    .functiontable IoCommands
CommandFunctionTableEnd:
CommandFunctionTable_SIZE = CommandFunctionTableEnd - CommandFunctionTable

.proc cmd__pause

    mov CONTROL, #0

    .assert T0OUT + 1 == T1OUT
    movw YA, T0OUT

    mov Y, #$ff
    bra __cmd__keyoff
.endproc

.proc cmd__pause_music_play_sfx

    mov CONTROL, #CONTROL__ENABLE_TIMER_1

    mov A, T0OUT

    mov Y, musicSfxChannelMask

    .assert PC == __cmd__keyoff
.endproc

.proc __cmd__keyoff
    mov A, #DSP_KOFF
    movw DSPADDR, YA

    ret
.endproc

.proc cmd__unpause

    mov CONTROL, #CONTROL__ENABLE_TIMER_0 | CONTROL__ENABLE_TIMER_1

    ret
.endproc

.proc cmd__set_main_volume
    mov songGlobals.mainVolume, Y

    mov A, #DSP_MVOL_L
    movw DSPADDR, YA

    mov A, #DSP_MVOL_R
    movw DSPADDR, YA

    ret
.endproc

.proc cmd__set_music_channels
    mov io_musicChannelsMask, Y

    mov A, Y
    eor A, #$ff
    tset1 keyOffShadow_music

    ret
.endproc

.proc cmd__set_song_timer

    dec Y
    cmp Y, #MIN_TICK_CLOCK - 1
    bcs Valid
        mov Y, #MIN_TICK_CLOCK - 1
Valid:
    inc Y

    mov T0TARGET, Y

    ret
.endproc

.proc cmd__set_global_music_volume

    inc Y
    mov globalVolume_music, Y

    mov volShadowDirty_music, #$ff

    ret
.endproc

.proc cmd__set_global_volumes
    call cmd__set_global_music_volume

    mov Y, DriverIO__PARAMETER_1_PORT

    .assert PC == cmd__set_sfx_volume
.endproc

.proc cmd__set_sfx_volume

    inc Y
    mov globalVolume_sfx, Y

    .assert N_SFX_CHANNELS == 2
    mov volShadowDirty_sfx, #%11000000

    ret
.endproc

.proc cmd__stop_sound_effects
    .assert N_SFX_CHANNELS == 2

    mov channelSoA_instructionPtr_h + FIRST_SFX_CHANNEL, #0
    mov channelSoA_instructionPtr_h + LAST_SFX_CHANNEL, #0

    mov X, #FIRST_SFX_CHANNEL
    call bc__disable_channel

    .assert FIRST_SFX_CHANNEL + 1 == LAST_SFX_CHANNEL
    inc X
    jmp bc__disable_channel
.endproc

.proc cmd__play_sound_effect
    .assert N_SFX_CHANNELS == 2
    BOTH_SFX_ACTIVE = (1 << 7) | (1 << 6)

    cmp Y, commonDataHeader.nSoundEffects
    bcs _PlaySfx_Return
        cmp Y, sfx_oneChannelSfxId+0
        beq sfx__maybe_restart_sfx_0

        cmp Y, sfx_oneChannelSfxId+1
        beq sfx__maybe_restart_sfx_1

        mov A, activeSoundEffects
        cmp A, #BOTH_SFX_ACTIVE
        bcs sfx__both_channels_active

        asl A
        bcc sfx__play_1
        bra sfx__play_sfx_0
.endproc

.proc sfx__both_channels_active
    .assert N_SFX_CHANNELS == 2
    BOTH_SFX_ACTIVE = (1 << 7) | (1 << 6)

    cmp Y, commonDataHeader.firstLowPrioritySfx
    bcs Return

        mov A, sfx_interruptibleFlags
        bne OtherTest
            cmp Y, commonDataHeader.nHighPrioritySfx
            bcc BothChannels
        OtherTest:
            cmp A, #BOTH_SFX_ACTIVE
            bcc NotBothChannels

        BothChannels:

            .assert N_SFX_CHANNELS == 2
            .assert sfx_remainingTicks__ELEMENT_SIZE == 2
            movw YA, sfx_remainingTicks+0
            cmpw YA, sfx_remainingTicks+2

            mov Y, DriverIO__PARAMETER_0_PORT

            bcc sfx__play_sfx_0
            bra sfx__play_1

        NotBothChannels:

            asl A
            bcs sfx__play_1
            bmi sfx__play_sfx_0

Return:
    ret
.endproc

_PlaySfx_Return = sfx__both_channels_active.Return

.proc sfx__maybe_restart_sfx_0
    bbc sfx_interruptibleFlags, 6, _PlaySfx_Return

    .assert PC == sfx__play_sfx_0
.endproc

.proc sfx__play_sfx_0

    _I = 0
    _BIT = 8 - N_SFX_CHANNELS + _I

    .assert sfx_remainingTicks__ELEMENT_SIZE == 2
    .assert sfx_oneChannelSfxId__ELEMENT_SIZE == 1

    mov A, [commonData.soundEffects_durationAndInterruptFlag_l] + Y
    mov sfx_remainingTicks + _I*2, A

    mov A, [commonData.soundEffects_durationAndInterruptFlag_h] + Y
    mov sfx_remainingTicks + _I*2 + 1, A

    asl A
    mov1 sfx_interruptibleFlags, _BIT, C

    mov sfx_oneChannelSfxId + _I, Y

    mov A, [commonData.soundEffects_addrAndOneChannelFlag_h] + Y
    bmi NotOneChannel
        mov sfx_oneChannelSfxId + _I, #$ff
    NotOneChannel:

    mov X, #FIRST_SFX_CHANNEL
    bra sfx__play_sfx_on_channel
.endproc

.proc sfx__maybe_restart_sfx_1
    bbc sfx_interruptibleFlags, 7, _PlaySfx_Return

    .assert PC == sfx__play_1
.endproc

.proc sfx__play_1

    _I = 1
    _BIT = 8 - N_SFX_CHANNELS + _I

    .assert sfx_remainingTicks__ELEMENT_SIZE == 2
    .assert sfx_oneChannelSfxId__ELEMENT_SIZE == 1

    mov A, [commonData.soundEffects_durationAndInterruptFlag_l] + Y
    mov sfx_remainingTicks + _I*2, A

    mov A, [commonData.soundEffects_durationAndInterruptFlag_h] + Y
    mov sfx_remainingTicks + _I*2 + 1, A

    asl A
    mov1 sfx_interruptibleFlags, _BIT, C

    mov sfx_oneChannelSfxId + _I, Y

    mov A, [commonData.soundEffects_addrAndOneChannelFlag_h] + Y
    bmi NotOneChannel
        mov sfx_oneChannelSfxId + _I, #$ff
    NotOneChannel:

    mov X, #LAST_SFX_CHANNEL

    .assert PC == sfx__play_sfx_on_channel
.endproc

.proc sfx__play_sfx_on_channel
    and A, #$7f
    mov channelSoA_instructionPtr_h + X, A

    mov A, [commonData.soundEffects_addrAndOneChannelFlag_l] + Y
    mov channelSoA_instructionPtr_l + X, A

    mov A, ChannelVoiceBit + X

    tset1 keyOffShadow_sfx

    tset1 keyOnMask_sfx

    tclr1 eonShadow_sfx

    tset1 activeSoundEffects

    eor A, #%11_000000
    tset1 volShadowDirty_sfx

    mov A, DriverIO__PARAMETER_1_PORT

    .assert PC == __reset_channel
.endproc

.proc __reset_channel

    setp
.p1

    .assert inFirstpage(channelSoA_pan)
    mov channelSoA_pan + X, A

    .assert inFirstpage(channelSoA_volume)
    mov A, #STARTING_VOLUME
    mov channelSoA_volume + X, A

    mov A, #0

    .assert inFirstpage(channelSoA_panEffect_direction)
    mov channelSoA_panEffect_direction + X, A

    .assert inFirstpage(channelSoA_volEffect_direction)
    mov channelSoA_volEffect_direction + X, A

    .assert inFirstpage(channelSoA_invertFlags)
    mov channelSoA_invertFlags + X, A

    .assert inFirstpage(channelSoA_earlyRelease_cmp)
    mov channelSoA_earlyRelease_cmp + X, A

    .assert inFirstpage(channelSoA_detune_l)
    .assert inFirstpage(channelSoA_detune_h)
    mov channelSoA_detune_l + X, A
    mov channelSoA_detune_h + X, A

    .assert page(channelSoA_transpose) != 0
    mov channelSoA_transpose + X, A

    clrp
.p0

    .assert inZeropage(channelSoA_virtualChannels_tempGain)
    mov channelSoA_virtualChannels_tempGain + X, A

    .assert inZeropage(channelSoA_portamento_direction)
    mov channelSoA_portamento_direction + X, A

    .assert inZeropage(channelSoA_vibrato_pitchOffsetPerTick)
    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    mov channelSoA_countdownTimer_h + X, A
    inc A

    mov channelSoA_countdownTimer_l + X, A

    mov A, BcStackIndexesStart + X
    mov channelSoA_stackPointer + X, A

    setc
    sbc A, #3
    mov channelSoA_loopStackPointer + X, A

    ret
.endproc

.proc process_music_channels
    incw songTickCounter

    .assert page(pendingNon_music) != 0
    mov A, pendingNon_music
    mov nonShadow_music, A

    mov A, keyOffShadow_music
    tclr1 pendingNon_music

    and A, musicSfxChannelMask
    mov Y, A
    mov A, #DSP_KOFF
    movw DSPADDR, YA

    process_song_globals

    mov volShadowDirty_tmp, volShadowDirty_music

    mov Y, #0
    mov keyOffShadow_music, Y
    mov volShadowDirty_music, Y

    .assert zpTmp == process_channels._mutedChannels

    mov A, noiseLock
    beq NoNoise
        mov Y, nonShadow_music
    NoNoise:
    mov zpTmp, Y

    mov A, voiceChannelsDirty_music
    and A, musicSfxChannelMask

    mov globalVolume_tmp, globalVolume_music
    mov Y, #FIRST_MUSIC_CHANNEL + 1
    mov X, #LAST_MUSIC_CHANNEL + 1
    call process_channels

    mov voiceChannelsDirty_music, voiceChannelsDirty_tmp

    ret
.endproc

.inline process_sfx_channels
    .assert N_SFX_CHANNELS == 2
    BOTH_SFX_ACTIVE = (1 << 7) | (1 << 6)

    mov sfxMusicEchoNoiseMask, prevActiveSoundEffects

    mov A, activeSoundEffects
    mov prevActiveSoundEffects, A

    or  A, keyOffShadow_sfx
    eor A, #$ff
    mov musicSfxChannelMask, A

    mov A, #DSP_KOFF
    mov Y, keyOffShadow_sfx
    movw DSPADDR, YA

    mov Y, pendingNon_sfx
    cmp Y, nonShadow_sfx
    beq SfxNoiseUnchanged

        mov nonShadow_sfx, Y

        mov A, Y
        bne SfxNoise

            or voiceChannelsDirty_music, nonShadow_music
            bra EndSfxNoiseIf

        SfxNoise:

            mov X, noiseLock
            bne EndNoiseLock
                mov A, nonShadow_music
                and A, musicSfxChannelMask
                beq EndNoiseLock
                    mov zpTmp, A

                    .assert VxVOL_L == 0

                    mov A, X
                    mov Y, A

                    VolLoop:
                        lsr zpTmp
                        bcc SkipVol
                            movw DSPADDR, YA
                            inc A
                            movw DSPADDR, YA
                            dec A

                            clrc
                    SkipVol:
                        adc A, #$10

                        mov X, zpTmp
                        bne VolLoop

                    mov Y, pendingNon_sfx
            EndNoiseLock:

            mov A, #0
            cmp Y, #BOTH_SFX_ACTIVE
            bcc EndSfxNoiseIf

                mov A, sfx_interruptibleFlags
                beq UseLastPlayNoiseChannel

                    eor A, #BOTH_SFX_ACTIVE

                    bne OneChannelUninterruptable
                        UseLastPlayNoiseChannel:

                            mov A, lastSfxPlayNoiseChannel
                    OneChannelUninterruptable:

                mov Y, A

                eor A, #BOTH_SFX_ACTIVE
        EndSfxNoiseIf:

        mov noiseLock, Y
        mov sfxMutedChannels, A

        tset1 voiceChannelsDirty_sfx
    SfxNoiseUnchanged:

    mov A, keyOffShadow_sfx
    tclr1 pendingNon_sfx

    mov volShadowDirty_tmp, volShadowDirty_sfx

    mov A, #0
    mov keyOffShadow_sfx, A
    mov volShadowDirty_sfx, A

    .assert page(sfxMutedChannels) != 0
    .assert zpTmp == process_channels._mutedChannels
    mov A, sfxMutedChannels
    mov zpTmp, A

    mov A, voiceChannelsDirty_sfx
    and A, activeSoundEffects

    mov globalVolume_tmp, globalVolume_sfx
    mov Y, #FIRST_SFX_CHANNEL + 1
    mov X, #LAST_SFX_CHANNEL + 1
    call process_channels

    mov voiceChannelsDirty_sfx, voiceChannelsDirty_tmp

    .assert N_SFX_CHANNELS == 2
    .assert sfx_remainingTicks__ELEMENT_SIZE == 2
    decw sfx_remainingTicks+0
    decw sfx_remainingTicks+2
.endinline

.proc process_channels
_mutedChannels = zpTmp

    mov _VoiceChannelLoop_CPX + 1, Y
    mov _BytecodeAndEffectLoop_CPX + 1, X

    mov voiceChannelsDirty_tmp, A

    VoiceChannelLoop:
        dec X

        asl voiceChannelsDirty_tmp
        bcc VoiceRegistersUnchanged
            mov A, LastVoiceRegister + X

            mov Y, channelSoA_virtualChannels_tempGain + X
            beq NoTempGain
                movw DSPADDR, YA
                mov Y, #0
                bra EndTempGain
            NoTempGain:

                .assert VxADSR1_ENABLE_BIT == 7
                mov Y, channelSoA_virtualChannels_adsr1 + X
                bpl Gain
                    dec A
                Gain:

                mov Y, channelSoA_virtualChannels_adsr2OrGain + X
                movw DSPADDR, YA

                mov Y, channelSoA_virtualChannels_adsr1 + X
            EndTempGain:

            .assert (VxGAIN  & %1111_0110) - 1 == VxADSR1
            .assert (VxADSR2 & %1111_0110) - 1 == VxADSR1
            and A, #%1111_0110
            dec A

            movw DSPADDR, YA

            dec A
            mov Y, channelSoA_virtualChannels_srcn + X
            movw DSPADDR, YA

            dec A
            mov Y, channelSoA_virtualChannels_pitch_h + X
            movw DSPADDR, YA

            dec A
            mov Y, channelSoA_virtualChannels_pitch_l + X
            movw DSPADDR, YA

            dec A
            mov Y, _mutedChannels
            bpl NotMuted
                mov Y, #0
                movw DSPADDR, YA

                bra EndMutedTest

            NotMuted:
                mov Y, channelSoA_virtualChannels_vol_r + X
                movw DSPADDR, YA

                mov Y, channelSoA_virtualChannels_vol_l + X
            EndMutedTest:
            dec A
            movw DSPADDR, YA

        VoiceRegistersUnchanged:

        asl _mutedChannels

    _VoiceChannelLoop_CPX:
        cmp X, #0
        bcs VoiceChannelLoop

    mov A, eonShadow_music
    eor A, eonShadow_sfx
    and A, sfxMusicEchoNoiseMask
    eor A, eonShadow_music
    mov Y, A
    mov A, #DSP_EON
    movw DSPADDR, YA

    mov A, nonShadow_music
    and A, musicSfxChannelMask
    or  A, nonShadow_sfx
    mov Y, A
    mov A, #DSP_NON
    movw DSPADDR, YA

    mov A, X
    bne SfxChannels

        mov A, #DSP_PMON
        mov Y, pmonShadow
        movw DSPADDR, YA

        mov A, noiseLock
        bne NoMusicNoise

            mov A, #DSP_FLG
            mov Y, noiseFreq_music
            movw DSPADDR, YA
        NoMusicNoise:

        mov A, keyOnShadow_music
        and A, musicSfxChannelMask
        and A, io_musicChannelsMask
        mov Y, A

        mov keyOnShadow_music, X

        bra EndMusicOrSfxIf

    SfxChannels:

        mov A, noiseLock
        beq NoSfxNoise
            .assert N_SFX_CHANNELS == 2
            bpl NoiseOnSfx0
                mov Y, noiseFreq_sfx+1
                bra EndSfxNoise
            NoiseOnSfx0:
                mov Y, noiseFreq_sfx+0
        EndSfxNoise:

            mov A, #DSP_FLG
            movw DSPADDR, YA
        NoSfxNoise:

        mov Y, keyOnShadow_sfx
        mov keyOnShadow_sfx, #0
    EndMusicOrSfxIf:

    mov A, #DSP_KON
    movw DSPADDR, YA

    BytecodeAndEffectLoop:

        lsr voiceChannelsDirty_tmp

        process_vibrato

        mov A, channelSoA_ticksAfterNote + X
        inc A
        bne NoTicksOverflow
            dec A
        NoTicksOverflow:
        mov channelSoA_ticksAfterNote + X, A

        mov A, channelSoA_countdownTimer_h + X
        beq CountdownHIsZero

            mov Y, channelSoA_countdownTimer_l + X
            bne NoCtHDec

                .assert page(channelSoA_countdownTimer_h) != 0
                dec A
                mov channelSoA_countdownTimer_h + X, A
            NoCtHDec:

            dec channelSoA_countdownTimer_l + X

            bra EndTimer

        CountdownHIsZero:

            dec channelSoA_countdownTimer_l + X
            beq CountdownZero

                mov Y, channelSoA_keyoffMsbFlag + X
                bpl EndTimer

                mov A, channelSoA_countdownTimer_l + X
                dec A
                beq KeyOff

                cmp A, channelSoA_earlyRelease_cmp + X
                bcs EndTimer

                mov A, channelSoA_ticksAfterNote + X
                cmp A, channelSoA_earlyRelease_minTicks + X
                bcc EndTimer

                    mov A, channelSoA_earlyRelease_gain + X
                    beq KeyOff
                        mov channelSoA_virtualChannels_tempGain + X, A

                        set1 voiceChannelsDirty_tmp, 7
                        bra EndTimer

                KeyOff:
                    mov A, #0

                    mov channelSoA_keyoffMsbFlag + X, A

                    mov channelSoA_virtualChannels_tempGain + X, A

                    mov A, ChannelVoiceBit + X
                    cmp X, #FIRST_SFX_CHANNEL
                    bcc WriteKeyOffMusicBits
                        tset1 keyOffShadow_sfx
                        tset1 keyOnMask_sfx
                        bra EndTimer

                    WriteKeyOffMusicBits:
                        tset1 keyOffShadow_music
                        tset1 keyOnMask_music
                        bra EndTimer

        CountdownZero:

            mov Y, channelSoA_instructionPtr_h + X
            beq ChannelDisabled

                lsr channelSoA_keyoffMsbFlag + X

                mov A, channelSoA_instructionPtr_l + X
                call process_bytecode

                movw YA, instructionPtr
                mov channelSoA_instructionPtr_l + X, A
                mov channelSoA_instructionPtr_h + X, Y

                set1 voiceChannelsDirty_tmp, 7
            ChannelDisabled:
        EndTimer:

        mov A, channelSoA_volEffect_direction + X
        beq NoVolEffect
            process_volume_effects

            set1 volShadowDirty_tmp, 7
        NoVolEffect:

        mov A, channelSoA_panEffect_direction + X
        beq NoPanEffect
            process_pan_effects

            set1 volShadowDirty_tmp, 7
        NoPanEffect:

        asl volShadowDirty_tmp
        bcc VolumeUnchanged
            update_vol_shadow

            set1 voiceChannelsDirty_tmp, 7
        VolumeUnchanged:

        process_portamento

        inc X

    _BytecodeAndEffectLoop_CPX:
        cmp X, #0
        bcs Return
            jmp BytecodeAndEffectLoop

Return:
    ret
.endproc

.inline process_song_globals
    mov A, songGlobalsDirty
    beq End
        .assert SONG_GLOBALS_DIRTY__FIR_FILTER_BIT == 7
        bpl FirUnchanged
            .assert DSP_C1 - DSP_C0 == $10
            .assert DSP_C7 < $80
            .assert DSP_C7 + $10 >= $80

            .assert SONG_GLOBALS_DIRTY__CLEAR_FIR_BIT == 0
            lsr A
            bcc NoClear

                clrc
                mov A, #DSP_C0
                mov Y, #0

                ClearFirLoop:

                    movw DSPADDR, YA

                    adc A, #$10
                    bpl ClearFirLoop
            NoClear:

            mov X, #0
            mov A, #DSP_C0

            FirLoop:
                mov Y, songGlobals.firFilter + X
                movw DSPADDR, YA
                inc X

                adc A, #$10
                bpl FirLoop

            mov A, songGlobalsDirty
        FirUnchanged:

        .assert SONG_GLOBALS_DIRTY__ECHO_VOLUME_BIT == 6
        asl A
        bpl EvolUnchanged
            mov DSPADDR, #DSP_EVOL_L

            mov A, songGlobals.echoVolume_l

            mov1 C, loaderDataType, LoaderDataType__STEREO_FLAG_BIT
            bcs Stereo

                adc A, songGlobals.echoVolume_r
                lsr A

                bbc songGlobals.echoInvertFlags, INVERT_FLAGS__MONO_BIT, NoMonoInvert
                    eor A, #$ff
                    inc A
                NoMonoInvert:
                mov DSPDATA, A

                bra EndMonoIf

            Stereo:
                bbc songGlobals.echoInvertFlags, INVERT_FLAGS__LEFT_BIT, NoLeftInvert
                    eor A, #$ff
                    inc A
                NoLeftInvert:
                mov DSPDATA, A

                mov A, songGlobals.echoVolume_r
                bbc songGlobals.echoInvertFlags, INVERT_FLAGS__RIGHT_BIT, NoRightInvert
                    eor A, #$ff
                    inc A
                NoRightInvert:

            EndMonoIf:

            .assert DSP_EVOL_L | (1 << 4) == DSP_EVOL_R
            set1 DSPADDR, 4
            mov DSPDATA, A
        EvolUnchanged:

        mov DSPADDR, #DSP_EFB
        mov DSPDATA, songGlobals.echoFeedback

        mov DSPADDR, #DSP_EDL
        mov DSPDATA, songGlobals.edl

        mov Y, songGlobals.mainVolume
        mov A, #DSP_MVOL_L
        movw DSPADDR, YA
        mov A, #DSP_MVOL_R
        movw DSPADDR, YA

        mov songGlobalsDirty, #0
    End:
.endinline

.inline process_volume_effects

    setp
.p1

    lsr A

    mov A, channelSoA_subVolume + X

    bcc Up

        sbc A, channelSoA_volEffect_offset_l + X
        mov channelSoA_subVolume + X, A

        mov A, channelSoA_volume + X
        sbc A, channelSoA_volEffect_offset_h + X

        bcs EndIf
            mov A, #0
            bra DisableEffect

        UpOverflow:
            mov A, #$ff

        DisableEffect:
            mov channelSoA_volume + X, A
            mov A, #0
            bra WriteDirection

    Up:

        adc A, channelSoA_volEffect_offset_l + X
        mov channelSoA_subVolume + X, A

        mov A, channelSoA_volume + X
        adc A, channelSoA_volEffect_offset_h + X

        bcs UpOverflow
    EndIf:

    mov channelSoA_volume + X, A

    dec channelSoA_volEffect_counter + X
    bne EndCounterIf
        mov A, channelSoA_volEffect_halfWavelength + X
        beq WriteDirection
            mov channelSoA_volEffect_counter + X, A

            mov A, channelSoA_volEffect_direction + X
            eor A, #1

    WriteDirection:
        mov channelSoA_volEffect_direction + X, A
    EndCounterIf:

    clrp
.p0

.endinline

.inline process_pan_effects

    setp
.p1

    lsr A

    mov A, channelSoA_subPan + X

    bcc Up

        sbc A, channelSoA_panEffect_offset_l + X
        mov channelSoA_subPan + X, A

        mov A, channelSoA_pan + X
        sbc A, channelSoA_panEffect_offset_h + X

        bcs EndIf
            mov A, #0
            bra DisableEffect

        UpOverflow:
            mov A, #MAX_PAN

        DisableEffect:
            mov channelSoA_pan + X, A
            mov A, #0
            bra WriteDirection

    Up:

        adc A, channelSoA_panEffect_offset_l + X
        mov channelSoA_subPan + X, A

        mov A, channelSoA_pan + X
        adc A, channelSoA_panEffect_offset_h + X

        bcs UpOverflow
        cmp A, #MAX_PAN + 1
        bcs UpOverflow
    EndIf:

    mov channelSoA_pan + X, A

    dec channelSoA_panEffect_counter + X
    bne EndCounterIf
        mov A, channelSoA_panEffect_halfWavelength + X
        beq WriteDirection
            mov channelSoA_panEffect_counter + X, A

            mov A, channelSoA_panEffect_direction + X
            eor A, #1
    WriteDirection:
        mov channelSoA_panEffect_direction + X, A
    EndCounterIf:

    clrp
.p0

.endinline

.inline update_vol_shadow
    .assert page(channelSoA_volume) != 0
    mov A, channelSoA_volume + X
    mov Y, A

    mov A, globalVolume_tmp
    beq NoMul
        mul YA
    NoMul:

    mov1 C, loaderDataType, LoaderDataType__STEREO_FLAG_BIT
    bcs Stereo

        mov A, Y
        lsr A
        lsr A
        mov Y, A

        .assert INVERT_FLAGS__MONO & %1100_0001 == 0
        mov A, channelSoA_invertFlags + X
        and A, #INVERT_FLAGS__MONO
        beq NoMonoInvert

            mov A, Y
            eor A, #$ff
            inc A
            mov Y, A
        NoMonoInvert:

        mov channelSoA_virtualChannels_vol_r + X, Y

        bra EndMonoIf

    Stereo:

        mov zpTmp, Y

        mov A, channelSoA_pan + X
        mul YA

        .assert INVERT_FLAGS__RIGHT_BIT == 7
        mov A, channelSoA_invertFlags + X
        bpl NoRightInvert

            mov A, Y
            eor A, #$ff
            inc A
            mov Y, A
        NoRightInvert:

        mov channelSoA_virtualChannels_vol_r + X, Y

        mov A, #MAX_PAN

        sbc A, channelSoA_pan + X
        mov Y, zpTmp
        mul YA

        .assert INVERT_FLAGS__LEFT_BIT == 6
        mov A, channelSoA_invertFlags + X
        asl A
        bpl NoLeftInvert

            mov A, Y
            eor A, #$ff
            inc A
            mov Y, A
        NoLeftInvert:
    EndMonoIf:

    mov channelSoA_virtualChannels_vol_l + X, Y
.endinline

.inline process_vibrato

    mov Y, channelSoA_portamento_direction + X
    bne NoVibrato

    mov Y, channelSoA_vibrato_pitchOffsetPerTick + X
    beq NoVibrato

    mov A, channelSoA_ticksAfterNote + X
    cmp A, channelSoA_vibrato_delay + X
    bcc NoVibrato

        mov A, channelSoA_vibrato_direction + X
        lsr A
        bcs Subtract

            mov A, Y

            adc A, channelSoA_virtualChannels_pitch_l + X
            bcc WritePitchL
                inc channelSoA_virtualChannels_pitch_h + X
                bra WritePitchL

        Subtract:

            mov A, channelSoA_virtualChannels_pitch_l + X

            sbc A, channelSoA_vibrato_pitchOffsetPerTick + X

            bcs WritePitchL
                dec channelSoA_virtualChannels_pitch_h + X

    WritePitchL:
        mov channelSoA_virtualChannels_pitch_l + X, A

        setp
    .p1

        dec channelSoA_vibrato_tickCounter + X
        bne TcNotZero
            mov A, channelSoA_vibrato_halfWavelength + X
            mov channelSoA_vibrato_tickCounter + X, A

            clrp
        .p0
            inc channelSoA_vibrato_direction + X
        TcNotZero:

        clrp
    .p0

        set1 voiceChannelsDirty_tmp, 7
    NoVibrato:
.endinline

.inline process_portamento
_target_h = zpTmp

    mov A, channelSoA_portamento_direction + X
    beq NoPortamento
        asl A

        mov A, channelSoA_portamento_target_h + X
        mov _target_h, A

        mov A, channelSoA_virtualChannels_pitch_l + X
        mov Y, channelSoA_virtualChannels_pitch_h + X

        bcc Up

            sbc A, channelSoA_portamento_speed + X
            bcs Down_CarryClear
                dec Y

                bmi MetTarget
            Down_CarryClear:

            cmp Y, _target_h
            bne Down_SkipCmpL
                cmp A, channelSoA_portamento_target_l + X
            Down_SkipCmpL:
            bcs WritePitch
            bra MetTarget

        Up:

            adc A, channelSoA_portamento_speed + X
            bcc Up_NoCarry
                inc Y

            Up_NoCarry:

            cmp Y, _target_h
            bne Up_SkipCmpL
                cmp A, channelSoA_portamento_target_l + X
        Up_SkipCmpL:
            bcc WritePitch

    MetTarget:

        mov A, #0
        mov channelSoA_portamento_direction + X, A

        mov channelSoA_vibrato_direction + X, A

        mov A, channelSoA_vibrato_tickCounterStart + X
        mov channelSoA_vibrato_tickCounter + X, A

        mov A, channelSoA_portamento_target_l + X
        mov Y, _target_h

    WritePitch:

        mov channelSoA_virtualChannels_pitch_l + X, A
        mov channelSoA_virtualChannels_pitch_h + X, Y

        set1 voiceChannelsDirty_tmp, 7
    NoPortamento:
.endinline

.proc _process_bytecode_with_loader_test__SecondBitTest

    bbc DriverIO__SWITCH_TO_LOADER_PORT, DriverIO__SWITCH_TO_LOADER_BIT, process_bytecode
    jmp LOADER_ADDR
.endproc

.proc process_bytecode_with_loader_test
    bbs DriverIO__SWITCH_TO_LOADER_PORT, DriverIO__SWITCH_TO_LOADER_BIT, _process_bytecode_with_loader_test__SecondBitTest

    .assert PC == process_bytecode
.endproc

.proc process_bytecode
    movw instructionPtr, YA

    .assert PC == process_next_bytecode
.endproc

.proc process_next_bytecode

    mov Y, #0
    mov A, [instructionPtr] + Y
    incw instructionPtr

    cmp A, #FIRST_PLAY_NOTE_INSTRUCTION
    bcs _bc__play_note

    mov Y, A
    mov A, BytecodeInstructionTable_h + Y
    push A
    mov A, BytecodeInstructionTable_l + Y
    push A

    cmp Y, #FIRST_NO_ARGUMENT_INSTRUCTION_OPCODE
    bcs NoParameters

        mov Y, #0
        mov A, [instructionPtr] + Y
        incw instructionPtr
    NoParameters:

    ret
.endproc

.proc bc__skip_last_loop_u16be
    mov zpTmpWord.h, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    bra _bc__skip_last_loop__bytesToSkip_l
.endproc

.proc bc__skip_last_loop_u8

    mov zpTmpWord.h, #0

    .assert PC == _bc__skip_last_loop__bytesToSkip_l
.endproc

.proc _bc__skip_last_loop__bytesToSkip_l
    mov zpTmpWord.l, A

    _bc__skip_last_loop__impl
.endproc

.proc bc__set_vibrato_depth_and_play_note
    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    .assert PC == _bc__play_note
.endproc

.proc _bc__play_note
_pitch = zpTmpWord
_pitch_l = zpTmpWord.l
_pitch_h = zpTmpWord.h

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    mov Y, channelSoA_virtualChannels_srcn + X

    clrc
    adc A, channelSoA_transpose + X
    asl A
    clrc

    adc A, [commonData.instruments_pitchOffset_l] + Y
    mov _pitch_l, A

    mov A, #0
    adc A, [commonData.instruments_pitchOffset_h] + Y
    mov _pitch_h, A

    mov Y, #1
    mov A, [_pitch] + Y
    clrc
    adc A, channelSoA_detune_l + X
    mov channelSoA_virtualChannels_pitch_l + X, A

    dec Y

    mov A, [_pitch] + Y
    adc A, channelSoA_detune_h + X

    .assert PC == _bc__play_note_set_pitch_h_and_read_length
.endproc

.proc _bc__play_note_set_pitch_h_and_read_length
    mov channelSoA_virtualChannels_pitch_h + X, A

    mov A, [instructionPtr] + Y
    bne ByteLength

        incw instructionPtr

        mov A, [instructionPtr] + Y
        mov channelSoA_countdownTimer_h + X, A
        incw instructionPtr

        mov A, [instructionPtr] + Y
    ByteLength:

    incw instructionPtr

    mov channelSoA_countdownTimer_l + X, A

    mov channelSoA_portamento_direction + X, Y

    .assert page(channelSoA_ticksAfterNote) != 0
    mov A, Y
    mov channelSoA_ticksAfterNote + X, A

    mov channelSoA_vibrato_direction + X, Y

    mov A, channelSoA_vibrato_tickCounterStart + X
    mov channelSoA_vibrato_tickCounter + X, A

    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel

        tclr1 pendingNon_sfx

        and A, keyOnMask_sfx
        tset1 keyOnShadow_sfx

        tclr1 keyOnMask_sfx

        ret

    MusicChannel:
        tclr1 pendingNon_music

        and A, keyOnMask_music
        tset1 keyOnShadow_music

        tclr1 keyOnMask_music

        ret
.endproc

.proc bc__play_pitch

    mov channelSoA_virtualChannels_pitch_l + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    bra _bc__play_note_set_pitch_h_and_read_length
.endproc

.proc bc__play_noise
    lsr A

    ror channelSoA_keyoffMsbFlag + X

    and A, #DSP_FLG__NOISE_FREQ_MASK
    mov Y, A

    mov A, #0
    mov channelSoA_ticksAfterNote + X, A

    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel

        mov noiseFreq_sfx - FIRST_SFX_CHANNEL + X, Y

        mov lastSfxPlayNoiseChannel, A
        tset1 pendingNon_sfx

        and A, keyOnMask_sfx
        tset1 keyOnShadow_sfx

        tclr1 keyOnMask_sfx

        jmp _bc__read_length

    MusicChannel:
        mov noiseFreq_music, Y

        tset1 pendingNon_music

        and A, keyOnMask_music
        tset1 keyOnShadow_music

        tclr1 keyOnMask_music

        jmp _bc__read_length
.endproc

.proc bc__keyon_next_note
    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel
        tset1 keyOnMask_sfx
        jmp process_next_bytecode

    MusicChannel:
        tset1 keyOnMask_music
        jmp process_next_bytecode
.endproc

.proc bc__portamento_down
    mov Y, #-2 & $ff

    .assert PC == bc__portamento_up
.endproc

.proc bc__portamento_up
_pitch = zpTmpWord
_pitch_l = zpTmpWord.l
_pitch_h = zpTmpWord.h

    inc Y

    mov channelSoA_portamento_direction + X, Y

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    mov Y, channelSoA_virtualChannels_srcn + X

    clrc
    adc A, channelSoA_transpose + X
    asl A
    clrc

    adc A, [commonData.instruments_pitchOffset_l] + Y
    mov _pitch_l, A

    mov A, #0
    adc A, [commonData.instruments_pitchOffset_h] + Y
    mov _pitch_h, A

    mov Y, #1
    mov A, [_pitch] + Y
    clrc
    adc A, channelSoA_detune_l + X
    mov channelSoA_portamento_target_l + X, A

    dec Y

    mov A, [_pitch] + Y
    adc A, channelSoA_detune_h + X

    .assert PC == _bc__portamento__read_speed_length
.endproc

.proc _bc__portamento__read_speed_length
    mov channelSoA_portamento_target_h + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr
    mov channelSoA_portamento_speed + X, A

    mov A, Y
    mov channelSoA_ticksAfterNote + X, A

    jmp _bc__read_length__y0
.endproc

.proc bc__portamento_pitch_down
    mov Y, #-2 & $ff

    .assert PC == bc__portamento_pitch_up
.endproc

.proc bc__portamento_pitch_up
    inc Y

    mov channelSoA_portamento_direction + X, Y

    mov channelSoA_portamento_target_l + X, A

    mov Y, #0
    mov A, [instructionPtr] + Y
    incw instructionPtr

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    bra _bc__portamento__read_speed_length
.endproc

.proc bc__portamento_pitch_calc
_tmp = zpTmpWord
_tmp_l = zpTmpWord.l

    mov channelSoA_portamento_target_l + X, A
    mov _tmp_l, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    jmp _bc__portamento_calc__read_lengths
.endproc

.proc bc__portamento_calc
_tmp = zpTmp
_pitch = zpTmp2
_pitch_l = zpTmp2
_pitch_h = zpTmp3
.assert _tmp < _pitch_l && _tmp < _pitch_h
.assert _pitch_h ==_pitch + 1

    lsr A

    ror channelSoA_keyoffMsbFlag + X

    mov Y, channelSoA_virtualChannels_srcn + X

    clrc
    adc A, channelSoA_transpose + X
    asl A
    clrc

    adc A, [commonData.instruments_pitchOffset_l] + Y
    mov _pitch_l, A

    mov A, #0
    adc A, [commonData.instruments_pitchOffset_h] + Y
    mov _pitch_h, A

    mov Y, #1
    mov A, [_pitch] + Y
    clrc
    adc A, channelSoA_detune_l + X
    mov channelSoA_portamento_target_l + X, A
    mov _tmp, A

    dec Y

    mov A, [_pitch] + Y
    adc A, channelSoA_detune_h + X

    .assert PC == _bc__portamento_calc__read_lengths
.endproc

.proc _bc__portamento_calc__read_lengths
_tmp = zpTmpWord
_tmp_l = zpTmpWord.l
_tmp_h = zpTmpWord.h
.assert _tmp == bc__portamento_calc._tmp

    mov channelSoA_portamento_target_h + X, A
    mov _tmp_h, A

    mov A, #0
    mov channelSoA_ticksAfterNote + X, A

    mov Y, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    push X
        push A

            dec Y

            mov channelSoA_portamento_direction + X, Y

            mov A, channelSoA_virtualChannels_pitch_l + X
            mov Y, channelSoA_virtualChannels_pitch_h + X

            subw YA, _tmp
            bpl NotNegative

                lsr channelSoA_portamento_direction + X

                movw _tmp, YA
                mov A, #0
                mov Y, A
                subw YA, _tmp
            NotNegative:
        pop X

        div YA, X
        bvc NoOverflow

            mov A, #$ff
            bra EndIf

        NoOverflow:

            mov Y, A
            bne EndIf
                inc A
        EndIf:
    pop X

    mov channelSoA_portamento_speed + X, A

    .assert PC == _bc__read_length
.endproc

.proc _bc__read_length
    mov Y, #0

    .assert PC == _bc__read_length__y0
.endproc

.proc _bc__read_length__y0

    mov A, [instructionPtr] + Y
    bne OneByteLength

        incw instructionPtr

        mov A, [instructionPtr] + Y
        mov channelSoA_countdownTimer_h + X, A
        incw instructionPtr

        mov A, [instructionPtr] + Y
    OneByteLength:

    incw instructionPtr
    mov channelSoA_countdownTimer_l + X, A

    ret
.endproc

.proc bc__set_vibrato
    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    mov A, Y
    bra _bc__set_vibrato_delay_and_wavelength
.endproc

.proc bc__set_vibrato_with_delay
    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    .assert PC == _bc__set_vibrato_delay_and_wavelength
.endproc

.proc _bc__set_vibrato_delay_and_wavelength
    mov channelSoA_vibrato_delay + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    mov channelSoA_vibrato_direction + X, Y

    mov channelSoA_vibrato_tickCounterStart + X, A
    mov channelSoA_vibrato_tickCounter + X, A

    asl A
    mov channelSoA_vibrato_halfWavelength + X, A

    jmp process_next_bytecode
.endproc

.proc bc__start_loop
_loopCount = zpTmp

    mov _loopCount, A

    mov A, channelSoA_stackPointer + X
    setc
    sbc A, #3

    cmp A, BcStackIndexesEnd + X
    bcc bc__disable_channel

    mov channelSoA_stackPointer + X, A
    mov channelSoA_loopStackPointer + X, A

    mov Y, A

    mov A, _loopCount
    mov bcStack + 0 + Y, A

    mov A, instructionPtr.l
    mov bcStack + 1 + Y, A

    mov A, instructionPtr.h
    mov bcStack + 2 + Y, A

    jmp process_next_bytecode
.endproc

.inline _bc__skip_last_loop__impl
_bytesToSkip = zpTmpWord

    mov Y, channelSoA_loopStackPointer + X

    mov A, bcStack+Y
    dec A
    bne process_next_bytecode

    mov A, Y
    clrc
    adc A, #3
    mov channelSoA_stackPointer + X, A

    cmp A, ValidLoopStackPointerTable + X
    bcs InvalidSp

        mov channelSoA_loopStackPointer + X, A
    InvalidSp:

    movw YA, _bytesToSkip
    addw YA, instructionPtr

    jmp process_bytecode_with_loader_test
.endinline

.proc bc__end_loop

    mov Y, channelSoA_loopStackPointer + X

    mov A, bcStack + 0 + Y
    dec A
    beq LastLoop
        mov bcStack + 0 + Y, A

        mov A, bcStack + 1 + Y
        mov instructionPtr.l, A

        mov A, bcStack + 2 + Y
        mov instructionPtr.h, A

        jmp process_next_bytecode

    LastLoop:

        mov A, Y
        clrc
        adc A, #3
        mov channelSoA_stackPointer + X, A

        cmp A, ValidLoopStackPointerTable + X
        bcs InvalidSp

            mov channelSoA_loopStackPointer + X, A
        InvalidSp:

        jmp process_next_bytecode
.endproc

.proc bc__disable_channel

    mov A, #$ff
    mov channelSoA_countdownTimer_h + X, A

    inc A

    mov instructionPtr.h, A

    mov channelSoA_portamento_direction + X, A

    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel

        tset1 keyOffShadow_sfx

        tclr1 eonShadow_sfx

        tclr1 activeSoundEffects

        mov A, #$ff
        mov sfx_oneChannelSfxId - N_MUSIC_CHANNELS + X, A

        ret

    MusicChannel:

        tset1 keyOffShadow_music

        ret
.endproc

.proc bc__goto_relative
    push A

    mov A, [instructionPtr] + Y
    mov Y, A

    pop A

    addw YA, instructionPtr

    jmp process_bytecode_with_loader_test
.endproc

.proc bc__call_subroutine_and_disable_vibrato

    mov channelSoA_vibrato_pitchOffsetPerTick + X, Y

    .assert PC == bc__call_subroutine
.endproc

.proc bc__call_subroutine
_subroutineId = zpTmp

    mov _subroutineId, A

    mov A, channelSoA_stackPointer + X
    dec A
    dec A
    cmp A, BcStackIndexesEnd + X
    bcc bc__disable_channel

    mov channelSoA_stackPointer + X, A

    mov Y, A

    mov A, instructionPtr.l
    mov bcStack + 0 + Y, A

    mov A, instructionPtr.h
    mov bcStack + 1 + Y, A

    mov Y, _subroutineId

    cmp X, #FIRST_SFX_CHANNEL
    bcs SfxSubroutine

        mov A, [subroutineTable_l] + Y
        adc A, songPtr.l
        mov instructionPtr.l, A

        mov A, [subroutineTable_h] + Y
        adc A, songPtr.h
        mov instructionPtr.h, A

        jmp process_next_bytecode

    SfxSubroutine:
        mov A, [commonData.sfxSubroutines_l] + Y
        mov instructionPtr.l, A

        mov A, [commonData.sfxSubroutines_h] + Y
        mov instructionPtr.h, A

        jmp process_next_bytecode
.endproc

.proc bc__return_from_subroutine_and_disable_vibrato

    mov A, #0
    mov channelSoA_vibrato_pitchOffsetPerTick + X, A

    .assert PC == bc__return_from_subroutine
.endproc

.proc bc__return_from_subroutine
    mov A, channelSoA_stackPointer + X
    inc A
    cmp A, BcStackIndexesStart + X
    bcs bc__disable_channel
    inc A

    mov channelSoA_stackPointer + X, A

    cmp A, ValidLoopStackPointerTable + X
    bcs InvalidSp

        mov channelSoA_loopStackPointer + X, A
    InvalidSp:

    mov Y, A

    mov A, bcStack - 2 + Y
    mov instructionPtr.l, A

    mov A, bcStack - 1 + Y
    mov instructionPtr.h, A

    jmp process_next_bytecode
.endproc

.proc bc__enable_echo
    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel
        tset1 eonShadow_sfx
        jmp process_next_bytecode

    MusicChannel:
        tset1 eonShadow_music
        jmp process_next_bytecode
.endproc

.proc bc__disable_echo
    mov A, ChannelVoiceBit + X

    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel
        tclr1 eonShadow_sfx
        jmp process_next_bytecode

    MusicChannel:
        tclr1 eonShadow_music
        jmp process_next_bytecode
.endproc

.proc bc__set_instrument

    mov channelSoA_virtualChannels_tempGain + X, Y

    mov Y, A

    mov channelSoA_virtualChannels_srcn + X, Y

    mov A, [commonData.instruments_adsr1] + Y
    mov channelSoA_virtualChannels_adsr1 + X, A

    mov A, [commonData.instruments_adsr2OrGain] + Y
    mov channelSoA_virtualChannels_adsr2OrGain + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_instrument_and_adsr_or_gain

    mov channelSoA_virtualChannels_tempGain + X, Y

    mov Y, A

    mov channelSoA_virtualChannels_srcn + X, Y

    mov Y, #0
    mov A, [instructionPtr] + Y
    mov channelSoA_virtualChannels_adsr1 + X, A
    incw instructionPtr

    mov A, [instructionPtr] + Y
    mov channelSoA_virtualChannels_adsr2OrGain + X, A
    incw instructionPtr

    jmp process_next_bytecode
.endproc

.proc bc__set_adsr

    mov channelSoA_virtualChannels_tempGain + X, Y

    mov channelSoA_virtualChannels_adsr1 + X, A

    mov A, [instructionPtr] + Y
    mov channelSoA_virtualChannels_adsr2OrGain + X, A
    incw instructionPtr

    jmp process_next_bytecode
.endproc

.proc bc__set_gain

    mov channelSoA_virtualChannels_tempGain + X, Y

    mov channelSoA_virtualChannels_adsr1 + X, Y

    mov channelSoA_virtualChannels_adsr2OrGain + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_early_release_no_minimum
    mov channelSoA_earlyRelease_cmp + X, A

    mov A, Y
    mov channelSoA_earlyRelease_minTicks + X, A

    bra _bc__set_early_release_gain
.endproc

.proc bc__set_early_release
    mov channelSoA_earlyRelease_cmp + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr
    mov channelSoA_earlyRelease_minTicks + X, A

    .assert PC == _bc__set_early_release_gain
.endproc

.proc _bc__set_early_release_gain

    mov A, [instructionPtr] + Y
    incw instructionPtr

    mov channelSoA_earlyRelease_gain + X, A

    jmp process_next_bytecode
.endproc

.proc bc__adjust_transpose
    clrc
    adc A, channelSoA_transpose + X

    .assert PC == bc__set_transpose
.endproc

.proc bc__set_transpose
    mov channelSoA_transpose + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_detune_i16
    mov channelSoA_detune_l + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    mov channelSoA_detune_h + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_detune_n8
    mov Y, #$ff

    .assert PC == bc__set_detune_p8
.endproc

.proc bc__set_detune_p8
    mov channelSoA_detune_l + X, A

    mov A, Y
    mov channelSoA_detune_h + X, A

    jmp process_next_bytecode
.endproc

.proc bc__adjust_pan
    clrc

    mov Y, A
    bpl Positive

        adc A, channelSoA_pan + X
        bcs EndIf
            mov A, #0
            bra EndIf

    Positive:

        adc A, channelSoA_pan + X

        bcs SetMax
            cmp A, #MAX_PAN + 1
            bcc EndIf
            SetMax:
                mov A, #MAX_PAN
    EndIf:

    .assert PC == bc__set_pan
.endproc

.proc bc__set_pan
    mov channelSoA_pan + X, A

    mov A, #0
    mov channelSoA_panEffect_direction + X, A

    set1 volShadowDirty_tmp, 7

    jmp process_next_bytecode
.endproc

.proc bc__set_pan_and_volume

    mov channelSoA_pan + X, A

    .assert page(channelSoA_panEffect_direction) != 0

    mov A, Y
    mov channelSoA_panEffect_direction + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    bra bc__set_volume
.endproc

.proc bc__adjust_volume
    clrc

    mov Y, A
    bpl Positive

        adc A, channelSoA_volume + X
        bcs EndIf
            mov A, #0
            bra EndIf

    Positive:

        adc A, channelSoA_volume + X
        bcc EndIf
            mov A, #$ff
    EndIf:

    .assert PC == bc__set_volume
.endproc

.proc bc__set_volume
    mov channelSoA_volume + X, A

    set1 volShadowDirty_tmp, 7

    mov A, #0
    mov channelSoA_volEffect_direction + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_channel_or_echo_invert
    .assert INVERT_FLAGS__MONO_BIT != 0

    asl A

    bbs loaderDataType, LoaderDataType__SURROUND_FLAG_BIT, Surround

        and A, #INVERT_FLAGS__MONO
        beq NoMonoInvert
            mov A, #$7f << 1
        NoMonoInvert:
    Surround:

    bcc SetEchoInvert

        mov channelSoA_invertFlags + X, A
        set1 volShadowDirty_tmp, 7

        jmp process_next_bytecode

    SetEchoInvert:

        mov songGlobals.echoInvertFlags, A
        set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__ECHO_VOLUME_BIT

        jmp process_next_bytecode
.endproc

.proc bc__volume_slide_up
    .assert inFirstpage(channelSoA_volEffect_counter)

    setp
.p1
    mov channelSoA_volEffect_counter + X, A

    mov A, #VOL_PAN_EFFECT_SLIDE_UP

    bra __bc__volume_slide__dp_true__
.endproc

.p0
.proc bc__volume_slide_down
    .assert inFirstpage(channelSoA_volEffect_counter)

    setp
.p1
    mov channelSoA_volEffect_counter + X, A

    mov A, #VOL_PAN_EFFECT_SLIDE_DOWN
    dec Y

    .assert PC == __bc__volume_slide__dp_true__
.endproc

.p1
.proc __bc__volume_slide__dp_true__
    .assert inFirstpage(channelSoA_volEffect_direction)

    mov channelSoA_volEffect_direction + X, A

    mov A, #0

    .assert PC == __bc__read_vol_effect_offset__dp_true__
.endproc

.p1
.proc __bc__read_vol_effect_offset__dp_true__
    .assert inFirstpage(channelSoA_volEffect_halfWavelength)
    .assert inFirstpage(channelSoA_subVolume)

    mov channelSoA_volEffect_halfWavelength + X, A
    mov channelSoA_subVolume + X, Y

    clrp
.p0

    mov Y, #0

    mov A, [instructionPtr] + Y
    mov channelSoA_volEffect_offset_l + X, A
    incw instructionPtr

    mov A, [instructionPtr] + Y
    mov channelSoA_volEffect_offset_h + X, A
    incw instructionPtr

    jmp process_next_bytecode
.endproc

.proc bc__tremolo
    .assert inFirstpage(channelSoA_volEffect_counter)
    .assert inFirstpage(channelSoA_volEffect_direction)

    setp
.p1

    mov Y, #VOL_PAN_EFFECT_TRIANGLE_UP
    mov channelSoA_volEffect_direction + X, Y

    mov channelSoA_volEffect_counter + X, A

    asl A

    mov Y, #$7f

    bra __bc__read_vol_effect_offset__dp_true__
.endproc
.p0

.proc bc__pan_slide_up
    .assert inFirstpage(channelSoA_panEffect_counter)

    setp
.p1

    mov channelSoA_panEffect_counter+X, A

    mov A, #VOL_PAN_EFFECT_SLIDE_UP

    bra __bc__pan_slide__dp_true__
.endproc
.p0

.proc bc__pan_slide_down
    .assert inFirstpage(channelSoA_panEffect_counter)

    setp
.p1

    mov channelSoA_panEffect_counter+X, A

    mov A, #VOL_PAN_EFFECT_SLIDE_DOWN
    dec Y

    .assert PC == __bc__pan_slide__dp_true__
.endproc
.p0

.p1
.proc __bc__pan_slide__dp_true__
    .assert inFirstpage(channelSoA_panEffect_direction)

    mov channelSoA_panEffect_direction + X, A

    mov A, #0

    .assert PC == __bc__read_pan_effect_offset__dp_true__
.endproc
.p0

.p1
.proc __bc__read_pan_effect_offset__dp_true__
    .assert inFirstpage(channelSoA_panEffect_halfWavelength)
    .assert inFirstpage(channelSoA_subPan)

    mov channelSoA_panEffect_halfWavelength + X, A
    mov channelSoA_subPan + X, Y

    clrp
.p0

    mov Y, #0

    mov A, [instructionPtr] + Y
    mov channelSoA_panEffect_offset_l + X, A
    incw instructionPtr

    mov A, [instructionPtr] + Y
    mov channelSoA_panEffect_offset_h + X, A
    incw instructionPtr

    jmp process_next_bytecode
.endproc

.proc bc__panbrello
    .assert inFirstpage(channelSoA_panEffect_counter)
    .assert inFirstpage(channelSoA_panEffect_direction)

    setp
.p1

    mov Y, #VOL_PAN_EFFECT_TRIANGLE_UP
    mov channelSoA_panEffect_direction + X, Y

    mov channelSoA_panEffect_counter + X, A

    asl A

    mov Y, #$7f
    bra __bc__read_pan_effect_offset__dp_true__
.endproc
.p0

.proc bc__reuse_temp_gain
    mov A, channelSoA_prevTempGain + X
    mov channelSoA_virtualChannels_tempGain + X, A

    jmp process_next_bytecode
.endproc

.proc bc__reuse_temp_gain_and_wait
    mov Y, A

    mov A, channelSoA_prevTempGain + X
    mov channelSoA_virtualChannels_tempGain + X, A

    mov A, Y
    bra bc__wait
.endproc

.proc bc__reuse_temp_gain_and_rest
    mov Y, A

    mov A, channelSoA_prevTempGain + X
    mov channelSoA_virtualChannels_tempGain + X, A

    mov A, Y

    bra bc__rest
.endproc

.proc bc__set_temp_gain
    mov channelSoA_virtualChannels_tempGain + X, A
    mov channelSoA_prevTempGain + X, A

    jmp process_next_bytecode
.endproc

.proc bc__set_temp_gain_and_wait
    mov channelSoA_virtualChannels_tempGain + X, A
    mov channelSoA_prevTempGain + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    bra bc__wait
.endproc

.proc bc__set_temp_gain_and_rest
    mov channelSoA_virtualChannels_tempGain + X, A
    mov channelSoA_prevTempGain + X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    .assert PC == bc__rest
.endproc

.proc bc__rest

    mov Y, #$80
    mov channelSoA_keyoffMsbFlag + X, Y

    .assert PC == bc__wait
.endproc

.proc bc__wait

    mov Y, A
    bne OneByteLength

        mov A, [instructionPtr] + Y
        incw instructionPtr
        mov channelSoA_countdownTimer_h + X, A

        mov A, [instructionPtr] + Y
        incw instructionPtr
    OneByteLength:

    mov channelSoA_countdownTimer_l + X, A

    ret
.endproc

.proc bc__set_echo_volume
    and A, #$7f
    mov songGlobals.echoVolume_l, A

    bra _bc__set_stereo_echo_volume_r
.endproc

.proc bc__set_stereo_echo_volume
    and A, #$7f
    mov songGlobals.echoVolume_l, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    and A, #$7f

    .assert PC == _bc__set_stereo_echo_volume_r
.endproc

.proc _bc__set_stereo_echo_volume_r
    mov songGlobals.echoVolume_r, A

    set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__ECHO_VOLUME_BIT

    jmp process_next_bytecode
.endproc

.proc bc__adjust_echo_volume
    push A

    call _bc__adjust_echo_volume_channel

    pop A
    bra _bc__adjust_stereo_echo_volume_r
.endproc

.proc bc__adjust_stereo_echo_volume

    call _bc__adjust_echo_volume_channel

    mov A, [instructionPtr] + Y
    incw instructionPtr

    .assert PC == _bc__adjust_stereo_echo_volume_r
.endproc

.proc _bc__adjust_stereo_echo_volume_r
    inc Y
    call _bc__adjust_echo_volume_channel

    set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__ECHO_VOLUME_BIT

    jmp process_next_bytecode
.endproc

.proc _bc__adjust_echo_volume_channel

    clrc
    adc A, songGlobals.echoVolume_l + Y

    bpl EndIf
        mov A, #0
        bvc EndIf
            mov A, #127
    EndIf:

    mov songGlobals.echoVolume_l + Y, A

    ret
.endproc

.proc bc__set_fir_filter
    mov songGlobals.firFilter + 0, A

    mov Y, #6
    Loop:
        mov A, [instructionPtr] + Y
        mov songGlobals.firFilter + 1 + Y, A
        dec Y
        bpl Loop

    inc Y
    mov A, #7

    addw YA, instructionPtr

    or songGlobalsDirty, #(1 << SONG_GLOBALS_DIRTY__FIR_FILTER_BIT) | (1 << SONG_GLOBALS_DIRTY__CLEAR_FIR_BIT)

    jmp process_bytecode
.endproc

.proc _bc__set_or_adjust_global_i8__out_of_bounds
    mov A, #MAX_SET_OR_ADJUST_GLOBAL_I8_PARAM

    .assert PC == bc__set_or_adjust_global_i8
.endproc

.proc bc__set_or_adjust_global_i8
    .assert songGlobals.firFilter + GLOBAL_I8_EFB_INDEX == songGlobals.echoFeedback
    .assert songGlobals.firFilter + GLOBAL_I8_MAIN_VOLUME_INDEX == songGlobals.mainVolume
    .assert MAX_SET_OR_ADJUST_GLOBAL_I8_PARAM >> 1 == GLOBAL_I8_MAIN_VOLUME_INDEX

    cmp A, #MAX_SET_OR_ADJUST_GLOBAL_I8_PARAM + 1
    bcs _bc__set_or_adjust_global_i8__out_of_bounds

    push X

    lsr A
    mov X, A

    mov A, [instructionPtr] + Y
    incw instructionPtr

    bcs SetI8

        adc A, songGlobals.firFilter + X
        bvc SetI8
            mov A, #I8_MIN & $ff
            bcs SetI8
                mov A, #I8_MAX
SetI8:
    mov songGlobals.firFilter + X, A

    cmp X, #GLOBAL_I8_EFB_INDEX

    pop X

    bcs EfbOrMvolDirty
        set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__FIR_FILTER_BIT
        jmp process_next_bytecode

    EfbOrMvolDirty:
        set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__EFB_OR_MVOL_BIT
        jmp process_next_bytecode
.endproc

.proc _bc__adjust_global_i8_limit__out_of_bounds
    mov A, #MAX_GLOBAL_I8_INDEX

    .assert PC == bc__adjust_global_i8_limit
.endproc

.proc bc__adjust_global_i8_limit
    .assert songGlobals.firFilter + GLOBAL_I8_EFB_INDEX == songGlobals.echoFeedback
    .assert songGlobals.firFilter + MAX_GLOBAL_I8_INDEX == songGlobals.mainVolume

    cmp A, #MAX_GLOBAL_I8_INDEX + 1
    bcs _bc__adjust_global_i8_limit__out_of_bounds

    push X
    mov X, A

    mov A, [instructionPtr] + Y
    bpl PositiveAdjust

        incw instructionPtr

        clrc
        adc A, songGlobals.firFilter + X
        bvs WriteLimit

        mov songGlobals.firFilter + X, A

        setc
        sbc A, [instructionPtr] + Y
        bvc LimitNegativeTest
            eor A, #$ff
            bra LimitNegativeTest

    PositiveAdjust:

        incw instructionPtr

        clrc
        adc A, songGlobals.firFilter + X
        bvs WriteLimit

        mov songGlobals.firFilter + X, A

        clrc
        sbc A, [instructionPtr] + Y
        bvs LimitNegativeTest
            eor A, #$ff

    LimitNegativeTest:
        bpl End

        WriteLimit:
            mov A, [instructionPtr] + Y
            mov songGlobals.firFilter + X, A
    End:

    incw instructionPtr

    cmp X, #GLOBAL_I8_EFB_INDEX

    pop X

    bcs bc__set_or_adjust_global_i8.EfbOrMvolDirty
        set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__FIR_FILTER_BIT
        jmp process_next_bytecode
.endproc

.proc bc__miscellaneous
    lsr A

    bcc Bit0Clear
        lsr A

        mov A, [instructionPtr] + Y
        incw instructionPtr

        bcs SetEchoDelay
            .assert SET_SONG_TICK_CLOCK_OPCODE == %01
            mbc__set_song_tick_clock
            jmp process_next_bytecode

        SetEchoDelay:
            .assert SET_ECHO_DELAY_OPCODE == %11
            mbc__set_echo_delay
            jmp process_next_bytecode

    Bit0Clear:
        .assert DISABLE_NOISE_OPCODE == 0
        bne PMod
            mbc__disable_noise__jmp_process_next_bytecode

        PMod:
            .assert DISABLE_PMOD_OPCODE & 1 == 0
            .assert ENABLE_PMOD_OPCODE & 1 == 0
            lsr A
            mbc__enable_or_disable_pmod
            jmp process_next_bytecode
.endproc

.inline mbc__set_echo_delay
    cmp A, maxEdl
    beq EndIf
    bcc EndIf
        mov A, maxEdl
    EndIf:

    mov songGlobals.edl, A

    set1 songGlobalsDirty, SONG_GLOBALS_DIRTY__EDL_BIT
.endinline

.inline mbc__set_song_tick_clock

    dec A
    cmp A, #MIN_TICK_CLOCK - 1
    bcs EndIf
        mov A, #MIN_TICK_CLOCK - 1
    EndIf:

    inc A

    mov T0TARGET, A
.endinline

.inline mbc__enable_or_disable_pmod
    mov A, ChannelVoiceBit + X
    and A, #ENABLE_PMOD_MASK

    tclr1 pmonShadow

    bcc Skip
        tset1 pmonShadow
    Skip:
.endinline

.inline mbc__disable_noise__jmp_process_next_bytecode
    mov A, ChannelVoiceBit + X
    cmp X, #FIRST_SFX_CHANNEL
    bcc MusicChannel
        tclr1 pendingNon_sfx
        jmp process_next_bytecode

    MusicChannel:
        tclr1 pendingNon_music
        jmp process_next_bytecode
.endinline

bc_ReservedForCustomUse = bc__disable_channel

BytecodeInstructionTable_l:
    .lofunctiontable INSTRUCTIONS_WITH_ARGUMENTS
BytecodeInstructionTable_l__NoArguments:
    .lofunctiontable NO_ARGUMENT_INSTRUCTIONS
BytecodeInstructionTable_l_End:

BytecodeInstructionTable_h:
    .hifunctiontable INSTRUCTIONS_WITH_ARGUMENTS
BytecodeInstructionTable_h__NoArguments:
    .hifunctiontable NO_ARGUMENT_INSTRUCTIONS
BytecodeInstructionTable_h_End:

.assert BytecodeInstructionTable_l__NoArguments - BytecodeInstructionTable_l == FIRST_NO_ARGUMENT_INSTRUCTION_OPCODE
.assert BytecodeInstructionTable_l_End - BytecodeInstructionTable_l == FIRST_PLAY_NOTE_INSTRUCTION

.assert BytecodeInstructionTable_h__NoArguments - BytecodeInstructionTable_h == FIRST_NO_ARGUMENT_INSTRUCTION_OPCODE
.assert BytecodeInstructionTable_h_End - BytecodeInstructionTable_h == FIRST_PLAY_NOTE_INSTRUCTION

ChannelVoiceBit:
    .db 1<<0, 1<<1, 1<<2, 1<<3, 1<<4, 1<<5, 1<<6, 1<<7, 1<<6, 1<<7
.assert PC - ChannelVoiceBit == N_CHANNELS

LastVoiceRegister:
    .db $07, $17, $27, $37, $47, $57, $67, $77, $67, $77
.assert PC - LastVoiceRegister == N_CHANNELS

BcStackIndexesEnd:
    .db  BC_CHANNEL_STACK_OFFSET
BcStackIndexesStart:
    .dbrepeat i in 0..N_CHANNELS, (i + 1) * BC_CHANNEL_STACK_SIZE + BC_CHANNEL_STACK_OFFSET
.assert PC - BcStackIndexesStart == N_CHANNELS
.assert BcStackIndexesEnd + 1 == BcStackIndexesStart

ValidLoopStackPointerTable:
    .dbrepeat i in 0..N_CHANNELS, (i + 1) * BC_CHANNEL_STACK_SIZE + BC_CHANNEL_STACK_OFFSET - 3 + 1
.assert PC - ValidLoopStackPointerTable == N_CHANNELS

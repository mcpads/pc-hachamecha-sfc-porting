MIN_TICK_CLOCK = 64

SFX_TICK_CLOCK = 64

N_MUSIC_CHANNELS = 8
N_SFX_CHANNELS = 2
N_CHANNELS = N_MUSIC_CHANNELS + N_SFX_CHANNELS

STARTING_VOLUME = 96

MAX_DIR_ITEMS = $100
MAX_INSTRUMENTS = $100

MAX_SOUND_EFFECTS = 254

COMMON_DATA_HEADER_SIZE = 24
COMMON_DATA_POINTERS_SIZE = 10 * 2

.struct DirItem
    start : ptr
    loop  : ptr
.endstruct

.struct CommonDataPointers

    instruments_pitchOffset_l : ptr
    instruments_pitchOffset_h : ptr

    instruments_adsr1 : ptr

    instruments_adsr2OrGain : ptr

    sfxSubroutines_l : ptr
    sfxSubroutines_h : ptr

    soundEffects_addrAndOneChannelFlag_l : ptr
    soundEffects_addrAndOneChannelFlag_h : ptr

    soundEffects_durationAndInterruptFlag_l : ptr
    soundEffects_durationAndInterruptFlag_h : ptr
.endstruct

.struct CommonDataHeader
    pointers       : CommonDataPointers

    _padding1      : u8

    nSoundEffects  : u8

    nHighPrioritySfx: u8

    firstLowPrioritySfx: u8

.endstruct

.struct GlobalSongSettings

    edl : u8

    firFilter : [i8 : 8]

    echoFeedback : i8

    mainVolume : i8

    echoVolume_l : u8
    echoVolume_r : u8

    echoInvertFlags : u8
.endstruct

.struct SongHeader

    activeMusicChannels : u8

    globals: GlobalSongSettings

    tickTimer : u8

    nSubroutines : u8

.endstruct

SONG_HEADER_SIZE = 17

    INVERT_FLAGS__RIGHT_BIT = 7
    INVERT_FLAGS__LEFT_BIT = 6

    INVERT_FLAGS__MONO_BIT = 1

    INVERT_FLAGS__MONO = 1 << INVERT_FLAGS__MONO_BIT

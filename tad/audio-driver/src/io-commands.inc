TAD_IO_VERSION = 23
LIVE_TRANSFER_BYTES = 96

    LoaderDataType__CODE        = 0

    LoaderDataType__COMMON_DATA = 1

    LoaderDataType__SONG_DATA_BIT = 7

    LoaderDataType__PLAY_SONG_BIT = 6

    LoaderDataType__RESET_GLOBAL_VOLUMES_BIT = 5

    LoaderDataType__STEREO_FLAG_BIT = 1

    LoaderDataType__SURROUND_FLAG_BIT = 0

    LoaderIO__LOADER_READY_L = %01001100
    LoaderIO__LOADER_READY_H = %01000100

        LoaderIO_InitToScpu__READY_PORT_L = CPUIO2
        LoaderIO_InitToScpu__READY_PORT_H = CPUIO3

        LoaderIO_InitToLoader__DATA_TYPE_PORT = CPUIO1

        LoaderIO_InitToLoader__READY_PORT_L = CPUIO2
        LoaderIO_InitToLoader__READY_PORT_H = CPUIO3

        LoaderIO_InitAckToScpu__ACK_VALUE = 0
        LoaderIO_InitAckToScpu__ACK_PORT = CPUIO3

        LoaderIO_TransferToLoader__DATA_PORT_L = CPUIO1
        LoaderIO_TransferToLoader__DATA_PORT_H = CPUIO2

        LoaderIO_TransferToLoader__SPINLOCK_PORT = CPUIO3

        LoaderIO_TransferToLoader__SPINLOCK_MASK = $0f
        LoaderIO_TransferToLoader__SPINLOCK_COMPLETE = $80
        LoaderIO_TransferToLoader__SPINLOCK_SWITCH_TO_LOADER_BIT = 5

        LoaderIO_TransferToScpu__SPINLOCK_ACK_PORT = CPUIO3

    DriverIO__COMMAND_PORT = CPUIO0

    DriverIO__N_COMMANDS = 15
    DriverIO__COMMAND_MASK   = %00011110
    DriverIO__COMMAND_I_MASK = %11100001

    DriverIO__PARAMETER_0_PORT = CPUIO1
    DriverIO__PARAMETER_1_PORT = CPUIO2

    DriverIO__SWITCH_TO_LOADER_PORT = CPUIO3
    DriverIO__SWITCH_TO_LOADER_BIT = 5

    DriverIO__COMMAND_ACK_PORT = CPUIO0

    DriverIO__MODE_PORT = CPUIO1

    DriverIO__MODE_IPL = $bb

    DriverIO__MODE_LOADER = $4c

    DriverIO__MODE_AUDIO_DRIVER = $61

IoCommands__PAUSE = 0

.ftdef IoCommands

    cmd__pause

    cmd__pause_music_play_sfx

    cmd__unpause

    cmd__play_sound_effect

    cmd__stop_sound_effects

    cmd__set_main_volume

    cmd__set_music_channels

    cmd__set_song_timer

    cmd__set_global_music_volume

    cmd__set_sfx_volume

    cmd__set_global_volumes

    cmd__stream_address
    cmd__stream_burst
    cmd__stream_play
    cmd__stream_stop
.endftdef

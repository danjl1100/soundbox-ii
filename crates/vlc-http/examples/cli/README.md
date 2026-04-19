# vlc-http Command Line Interface

Quickly experiment with VLC commands, using the clap interface.

1. Launch VLC with web interface:
    ```shell
    cd soundbox-ii
    ./dev_server.sh vlc
    ```
1. Connect to VLC using the cli example:
    ```shell
    cd soundbox-ii/vlc-http
    direnv exec ../../soundbox-ii cargo r --example cli
    ```

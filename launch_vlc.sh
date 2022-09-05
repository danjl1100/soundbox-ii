if [ "${VLC_BIND_HOST}" = "" ]; then
  echo "VLC_BIND_HOST is not set";
  exit 1;
fi
if [ "${VLC_PORT}" = "" ]; then
  echo "VLC_PORT is not set";
  exit 1;
fi
if [ "${VLC_PASSWORD}" = "" ]; then
  echo "VLC_PASSWORD is not set";
  exit 1;
fi

ARGS="--audio-replay-gain-mode track --http-host ${VLC_BIND_HOST} --http-port ${VLC_PORT} --http-password ${VLC_PASSWORD}"

case "$1" in
  "macos")
    echo "!!!NOTE!!! Need to click menu:  View > Add Interface > Web"
    echo ""
    echo "Press enter to launch visual interface (macos app)"
    read a
    open /Applications/VLC.app --args ${ARGS}
    ;;
  "-v")
    echo "!!!NOTE!!! Need to click menu:  View > Add Interface > Web"
    echo ""
    echo "Press enter to launch visual interface"
    read a
    vlc ${ARGS}
    ;;
  "")
    cvlc --intf http ${ARGS}
    ;;
  *)
    echo "Unknown option \"$1\""
    exit 1
    ;;
esac

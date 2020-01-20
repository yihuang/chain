#!/bin/bash
set -e
cd "$(dirname "${BASH_SOURCE[0]}")"

# cleanup first
./cleanup.sh

# ensure dependencies for integration tests
./deps.sh
PYTHON_VENV_DIR=${PYTHON_VENV_DIR:-"./bot/.venv"}
source $PYTHON_VENV_DIR/bin/activate

# prepare chain binaries
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"../target"}
ln -sf $CARGO_TARGET_DIR/debug/tx_query_enclave.signed.so .
ln -sf $CARGO_TARGET_DIR/debug/tx_validation_enclave.signed.so .
export PATH=$CARGO_TARGET_DIR/debug:$PATH

# environment variables for integration tests
export PASSPHRASE=123456
export BASE_PORT=${BASE_PORT:-26650}
export CLIENT_RPC_PORT=$(($BASE_PORT + 9))

function wait_http() {
    for i in $(seq 0 10);
    do
        curl -s "http://127.0.0.1:$1" > /dev/null
        if [ $? -eq 0 ]; then
            return 0
        fi
        sleep 2
    done
    return 1
}

function runtest() {
    echo "Preparing... $1"
    chainbot.py prepare multinode/$1_cluster.json --base_port $BASE_PORT

    echo "Startup..."
    supervisord -n -c data/tasks.ini &
    if ! wait_http $CLIENT_RPC_PORT; then
        echo 'client-rpc of first node still not ready, giveup.'
        cat data/logs/*.log
        RETCODE=1
    else
        set +e
        python -u ./multinode/$1_test.py
        RETCODE=$?
        set -e
    fi

    if [ $RETCODE -ne 0 ]; then
        tail -n 100 data/logs/*.log
    fi

    echo "Quit supervisord..."
    kill -QUIT `cat data/supervisord.pid`
    wait
    rm -r data
    rm supervisord.log

    return $RETCODE
}

if [ -d data ]; then
    echo "Last run doesn't quit cleanly, please quit supervisord daemon and remove integration-tests/data manually."
    exit 1;
fi

runtest "jail"
runtest "join"

./cleanup.sh

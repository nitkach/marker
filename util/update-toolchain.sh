#!/bin/bash

if [[ $1 == nightly-????-??-?? ]]
then
    sed -i "s/nightly-2023-08-24/$1/g" ./marker_rustc_driver/src/main.rs ./marker_rustc_driver/README.md ./rust-toolchain.toml ./.github/workflows/* ./util/update-toolchain.sh ./cargo-marker/src/backend/driver.rs ./cargo-marker/README.md
else
    echo "Please enter a valid toolchain like \`nightly-2022-01-01\`"
fi;

# Publishing a new release

1. Update the code.

    ```bash
    # make sure we don't include personal information (such as our
    # home directory name) in the release
    cd /tmp

    # make sure we don't include any untracked files in the release
    git clone --recurse-submodules git@github.com:stevenengler/transportal.git
    cd transportal

    # update the version
    vim Cargo.toml
    cargo update --package transportal

    # check for errors
    git diff
    cargo publish --dry-run --allow-dirty

    # add and commit version changes with commit message, for example
    # "Updated version to '0.2.1'"
    git add --patch
    git commit
    git push
    ```

2. After CI tests finish on GitHub, mark it as a new release.

3. Publish the crate.

    ```bash
    # remove symlinks to vendored files
    cp --remove-destination "$(dirname static/js/htmx.js)/$(readlink static/js/htmx.js)" static/js/htmx.js
    cp --remove-destination "$(dirname static/js/sse.js)/$(readlink static/js/sse.js)" static/js/sse.js

    # remove vendored repositories
    rm -r vendored/

    # make sure there are no unexpected untracked or changed files
    git status

    # publish
    cargo publish --allow-dirty --dry-run
    cargo publish --allow-dirty
    ```

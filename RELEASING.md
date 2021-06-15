# Release Checklist

These steps assume that you've checked out the Soketto repository and are in the root directory of it.

1. Ensure that everything you'd like to see released is on the `develop` branch.

2. Create a release branch off `develop`, for example `release-v0.6.0`. The branch name should start with `release` 
   so that we can target commits with CI. Decide how far the version needs to be bumped based on the changes to date. 
   If unsure what to bump the version to (e.g. is it a major, minor or patch release), check with the Parity Tools team.

3. Check that you're happy with the current documentation.
   
   ```
   cargo doc --open
   ```

   CI checks for broken internal links at the moment. Optionally you can also confirm that any external links
   are still valid like so:

   ```
   cargo install cargo-deadlinks
   cargo deadlinks --check-http
   ```

   If there are minor issues with the documentation, they can be fixed in the release branch.

4. Bump the crate version in `Cargo.toml` to whatever was decided in step 2.

5. Update `CHANGELOG.md` to reflect the difference between this release and last. If you're unsure of
   what to add, check with the Tools team. 
   
   One way to gain some inspiration about what to write is by looking through the commit history since 
   the last version (eg `git log --pretty LAST_VERSION_TAG..HEAD`).

   Alternately, look at the commit history: https://github.com/paritytech/soketto/commits/develop.

6. Commit any of the above changes to the release branch and open a PR in GitHub with a base of `master`.

7. Once the branch has been reviewed and passes CI, it can be merged to `master`.

8. Now, we're ready to publish the release to crates.io.

   Checkout `master`, ensuring we're looking at that latest merge (`git pull`). 
   
   Next, do a dry run to make sure that things seem sane:
   ```
   cargo publish --dry-run
   ```

   If we're happy with everything, proceed with the release:
   ```
   cargo publish
   ```

8. If the release was successful, then tag the commit that we released in the `master` branch with the
   version that we just released, for example:

   ```
   git tag v0.6.0 # use the version number you've just published to crates.io, not this one
   git push --tags
   ```

9. Merge the `master` branch back to develop so that we keep track of any changes that we made on
   the release branch.

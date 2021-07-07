use crate::file::{store_writable, transaction::prepare_and_commit::empty_store};
use git_hash::ObjectId;
use git_lock::acquire::Fail;
use git_ref::{
    file::WriteReflog,
    mutable::Target,
    transaction::{Change, RefEdit, RefLog},
};
use std::convert::TryInto;

#[test]
fn delete_a_ref_which_is_gone_succeeds() -> crate::Result {
    let (_keep, store) = empty_store(WriteReflog::Normal)?;
    let edits = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: None,
                    mode: RefLog::AndReference,
                },
                name: "DOES_NOT_EXIST".try_into()?,
                deref: false,
            }),
            Fail::Immediately,
        )
        .commit()?;
    assert_eq!(edits.len(), 1);
    Ok(())
}

#[test]
fn delete_a_ref_which_is_gone_but_must_exist_fails() -> crate::Result {
    let (_keep, store) = empty_store(WriteReflog::Normal)?;
    let res = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: Some(Target::Peeled(ObjectId::null_sha1())),
                    mode: RefLog::AndReference,
                },
                name: "DOES_NOT_EXIST".try_into()?,
                deref: false,
            }),
            Fail::Immediately,
        )
        .commit();
    match res {
        Ok(_) => unreachable!("must exist, but it doesn't actually exist"),
        Err(err) => assert_eq!(
            err.to_string(),
            "The reference 'DOES_NOT_EXIST' for deletion did not exist"
        ),
    }
    Ok(())
}

#[test]
fn delete_ref_and_reflog_on_symbolic_no_deref() -> crate::Result {
    let (_keep, store) = store_writable("make_repo_for_reflog.sh")?;
    let head = store.find_one_existing("HEAD")?;
    assert!(head.log_exists().unwrap());
    let _main = store.find_one_existing("main")?;

    let edits = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: Some(Target::Peeled(ObjectId::null_sha1())),
                    mode: RefLog::AndReference,
                },
                name: head.name().into(),
                deref: false,
            }),
            Fail::Immediately,
        )
        .commit()?;

    assert_eq!(
        edits,
        vec![RefEdit {
            change: Change::Delete {
                previous: Some(Target::Symbolic("refs/heads/main".try_into()?)),
                mode: RefLog::AndReference,
            },
            name: head.name().into(),
            deref: false
        }],
        "the previous value was updated with the actual one"
    );
    assert!(
        store.reflog_iter_rev("HEAD", &mut [0u8; 128])?.is_none(),
        "reflog was deleted"
    );
    assert!(store.find_one("HEAD")?.is_none(), "ref was deleted");
    assert!(store.find_one("main")?.is_some(), "referent still exists");
    Ok(())
}

#[test]
fn delete_ref_with_incorrect_previous_value_fails() {
    let (_keep, store) = store_writable("make_repo_for_reflog.sh").unwrap();
    let head = store.find_one_existing("HEAD").unwrap();
    assert!(head.log_exists().unwrap());

    let err = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: Some(Target::Symbolic("refs/heads/main".try_into().unwrap())),
                    mode: RefLog::Only,
                },
                name: head.name().into(),
                deref: true,
            }),
            Fail::Immediately,
        )
        .commit()
        .expect_err("mismatch is detected");

    assert_eq!(err.to_string(), "The reference 'refs/heads/main' should have content ref: refs/heads/main, actual content was 02a7a22d90d7c02fb494ed25551850b868e634f0");
    // everything stays as is
    let head = store.find_one_existing("HEAD").unwrap();
    assert!(head.log_exists().unwrap());
    let main = store.find_one_existing("main").expect("referent still exists");
    assert!(main.log_exists().unwrap());
}

#[test]
fn delete_reflog_only_of_symbolic_no_deref() -> crate::Result {
    let (_keep, store) = store_writable("make_repo_for_reflog.sh")?;
    let head = store.find_one_existing("HEAD")?;
    assert!(head.log_exists().unwrap());

    let edits = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: Some(Target::Symbolic("refs/heads/main".try_into()?)),
                    mode: RefLog::Only,
                },
                name: head.name().into(),
                deref: false,
            }),
            Fail::Immediately,
        )
        .commit()?;

    assert_eq!(edits.len(), 1);
    let head = store.find_one_existing("HEAD")?;
    assert!(!head.log_exists().unwrap());
    let main = store.find_one_existing("main").expect("referent still exists");
    assert!(main.log_exists()?, "log is untouched, too");
    assert_eq!(
        main.target(),
        head.peel_one_level().expect("a symref")?.target(),
        "head points to main"
    );
    Ok(())
}

#[test]
fn delete_reflog_only_of_symbolic_with_deref() -> crate::Result {
    let (_keep, store) = store_writable("make_repo_for_reflog.sh")?;
    let head = store.find_one_existing("HEAD")?;
    assert!(head.log_exists()?);

    let edits = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: Some(Target::Peeled(ObjectId::null_sha1())),
                    mode: RefLog::Only,
                },
                name: head.name().into(),
                deref: true,
            }),
            Fail::Immediately,
        )
        .commit()?;

    assert_eq!(edits.len(), 2);
    let head = store.find_one_existing("HEAD")?;
    assert!(!head.log_exists()?);
    let main = store.find_one_existing("main").expect("referent still exists");
    assert!(!main.log_exists()?, "log is removed");
    assert_eq!(
        main.target(),
        head.peel_one_level().expect("a symref")?.target(),
        "head points to main"
    );
    Ok(())
}

#[test]
#[ignore]
/// Based on https://github.com/git/git/blob/master/refs/files-backend.c#L514:L515
fn delete_broken_ref_that_must_exist_fails_as_it_is_no_valid_ref() {}

#[test]
#[should_panic]
/// Based on https://github.com/git/git/blob/master/refs/files-backend.c#L514:L515
fn delete_broken_ref_that_may_not_exist_works_even_in_deref_mode() {
    let (_keep, store) = empty_store(WriteReflog::Normal).unwrap();
    std::fs::write(store.base.join("HEAD"), &b"broken").unwrap();
    assert!(store.find_one("HEAD").is_err(), "the ref is truly broken");

    let edits = store
        .transaction(
            Some(RefEdit {
                change: Change::Delete {
                    previous: None,
                    mode: RefLog::AndReference,
                },
                name: "HEAD".try_into().unwrap(),
                deref: true,
            }),
            Fail::Immediately,
        )
        .commit()
        .unwrap();

    assert!(store.find_one("HEAD").unwrap().is_none(), "the ref was deleted");
    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits,
        vec![RefEdit {
            change: Change::Delete {
                previous: None,
                mode: RefLog::AndReference,
            },
            name: "HEAD".try_into().unwrap(),
            deref: false,
        }]
    );
    assert_eq!(edits[0].change.previous(), None, "the previous value could not be read");
}

#[test]
fn store_write_mode_has_no_effect_and_reflogs_are_always_deleted() -> crate::Result {
    for reflog_writemode in &[git_ref::file::WriteReflog::Normal, git_ref::file::WriteReflog::Disable] {
        let (_keep, mut store) = store_writable("make_repo_for_reflog.sh")?;
        store.write_reflog = *reflog_writemode;
        assert!(store.find_one_existing("HEAD")?.log_exists()?,);
        let edits = store
            .transaction(
                Some(RefEdit {
                    change: Change::Delete {
                        previous: None,
                        mode: RefLog::Only,
                    },
                    name: "HEAD".try_into()?,
                    deref: false,
                }),
                Fail::Immediately,
            )
            .commit()?;
        assert_eq!(edits.len(), 1);
        assert!(!store.find_one_existing("HEAD")?.log_exists()?, "log was deleted");
    }
    Ok(())
}

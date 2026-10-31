//! Actual operator HTTP/PG assertions; called by the existing native deployment
//! integration after importing its six real frozen model originals.
use super::*;

pub(super) async fn exercise(
    app: &axum::Router,
    pool: &std::sync::Arc<brassclaw_pg::PgPool>,
    specification: &Value,
    detail: &Value,
) {
    use sha2::{Digest, Sha256};
    let observed: Value =
        serde_json::from_str(detail["format_observation"].as_str().unwrap()).unwrap();
    let template = observed["templates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["file"] == "chat_template.jinja")
        .unwrap();
    let files = detail["summary"]["files"].as_array().unwrap();
    let file = |name| {
        files.iter().find(|row| row["name"] == name).unwrap()["sha256"]
            .as_str()
            .unwrap()
    };
    let document = crate::prefix_pair_construction::tests::draft(
        specification["id"].as_str().unwrap(),
        detail["summary"]["id"].as_str().unwrap(),
        "chat_template.jinja",
        file("chat_template.jinja"),
        Value::Null,
        template["template_sha256"].as_str().unwrap(),
        file("config.json"),
    );
    let request =
        json!({"specification":specification["id"],"proposal_bytes":document.to_string()});
    let route = "/api/webchat/v2/prefix-model-designs/proposals";
    let before = at(
        app,
        "GET",
        "/api/webchat/v2/prefix-model-designs",
        None,
        true,
    )
    .await
    .1;
    assert_eq!(
        at(app, "POST", route, Some(&request), false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, retained) = at(app, "POST", route, Some(&request), true).await;
    assert_eq!(status, StatusCode::OK, "{retained}");
    assert_eq!(
        retained["qualification"],
        "unaccepted-pair-construction-proposal"
    );
    assert!(retained.get("profile").is_none());
    assert_eq!(retained["proposal_bytes"], request["proposal_bytes"]);
    assert_eq!(
        at(app, "POST", route, Some(&request), true).await.1,
        retained
    );
    let packet_route = format!(
        "{route}/{}/{}/cases",
        specification["id"].as_str().unwrap(),
        retained["id"].as_str().unwrap()
    );
    assert_eq!(
        at(app, "GET", &packet_route, None, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, packet) = at(app, "GET", &packet_route, None, true).await;
    assert_eq!(status, StatusCode::OK, "{packet}");
    assert_eq!(packet["proposal"], retained["id"]);
    assert_eq!(packet["qualification"], "unqualified-case-input");
    let cases: Value = serde_json::from_str(packet["case_packet_bytes"].as_str().unwrap()).unwrap();
    assert_eq!(
        cases["cases"].as_array().unwrap().len(),
        document["worked_cases"].as_array().unwrap().len()
    );
    assert_eq!(
        cases["cases"][0]["context_json"],
        document["worked_cases"][0]["render_context_json"]
    );
    assert_eq!(
        cases["cases"][0]["expected_render"],
        document["worked_cases"][0]["expected_request"]
    );
    assert_eq!(at(app, "GET", &packet_route, None, true).await.1, packet);
    for (pointer, replacement) in [
        ("/template/template_sha256", json!("0".repeat(64))),
        ("/evidence/0/file_sha256", json!("0".repeat(64))),
        ("/evidence/1/pointer", json!("/unretained-declaration")),
        ("/construction/join", json!("server-injection")),
        (
            "/worked_cases/0/expected_request",
            json!("silently rewritten"),
        ),
    ] {
        let mut invalid = document.clone();
        *invalid.pointer_mut(pointer).unwrap() = replacement;
        let input =
            json!({"specification":specification["id"],"proposal_bytes":invalid.to_string()});
        assert_eq!(
            at(app, "POST", route, Some(&input), true).await.0,
            StatusCode::BAD_REQUEST,
            "{pointer}"
        );
    }
    let mut copied = document.clone();
    copied["construction"]["max_prefix_tokens"] = json!(100000);
    assert_eq!(
        at(
            app,
            "POST",
            route,
            Some(&json!({"specification":specification["id"],"proposal_bytes":copied.to_string()})),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        at(
            app,
            "POST",
            "/api/webchat/v2/prefixes/base-prompt/design-proposals",
            Some(&request),
            true
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let spec_bytes: Value =
        serde_json::from_str(specification["specification_bytes"].as_str().unwrap()).unwrap();
    let foreign = crate::prefix_model_designs::PgPrefixModelDesigns::new(
        pool.clone(),
        "foreign-pair-owner".into(),
    );
    assert!(
        foreign
            .case_packet(
                specification["id"].as_str().unwrap(),
                retained["id"].as_str().unwrap()
            )
            .await
            .is_err()
    );
    assert!(
        foreign
            .retain_pair_proposal(serde_json::from_value(request.clone()).unwrap())
            .await
            .is_err()
    );
    let after = at(
        app,
        "GET",
        "/api/webchat/v2/prefix-model-designs",
        None,
        true,
    )
    .await
    .1;
    for unchanged in ["targets", "specifications", "proposals", "artifact_sets"] {
        assert_eq!(after[unchanged], before[unchanged], "{unchanged}");
    }
    assert_eq!(after["pair_proposals"], json!([retained]));
    assert_eq!(after["design_generation_available"], false);
    let own = crate::prefix_model_designs::PgPrefixModelDesigns::new(
        pool.clone(),
        spec_bytes["provider_namespace"].as_str().unwrap().into(),
    );
    assert!(
        own.get_proposal("base-prompt", retained["id"].as_str().unwrap())
            .await
            .is_err(),
        "pair draft is not a role proposal"
    );
    let client = pool.get().await.unwrap();
    let id = retained["id"].as_str().unwrap();
    assert_eq!(client.query_one("SELECT count(*) FROM reborn_prefix_design_proposals WHERE proposal_id=$1 AND profile_name IS NULL",&[&id]).await.unwrap().get::<_,i64>(0),1);
    assert!(client.execute("UPDATE reborn_prefix_design_proposals SET profile_name='base-prompt' WHERE proposal_id=$1",&[&id]).await.is_err());
    let mut wrong_scope = document.clone();
    wrong_scope["rationale"] =
        json!("A pair draft cannot acquire a profile through direct insertion");
    let wrong_bytes = wrong_scope.to_string();
    let wrong_id = hex::encode(Sha256::digest(wrong_bytes.as_bytes()));
    let error=client.execute("INSERT INTO reborn_prefix_design_proposals(proposal_id,profile_name,specification_id,proposal_bytes) VALUES($1,'base-prompt',$2,$3)",
        &[&wrong_id,&specification["id"].as_str().unwrap(),&wrong_bytes]).await.unwrap_err();
    assert_eq!(
        error.as_db_error().unwrap().constraint(),
        Some("prefix_proposal_scope")
    );
    // Even direct SQL cannot use the old per-profile acceptance contract for a
    // shared NULL-profile draft; no acceptance writer is added by this package.
    let candidate = client
        .query_one(
            "SELECT profile_name,generation_id FROM reborn_prefix_generations LIMIT 1",
            &[],
        )
        .await
        .unwrap();
    // V183 separates proposal scope from profile identity. Both constraints
    // reject this legacy-profile acceptance of a pair draft; PostgreSQL need
    // not report those two independent foreign-key checks in a fixed order.
    let expected_fks = [
        "prefix_acceptance_scoped_proposal",
        "prefix_acceptance_profile_proposal",
    ];
    let profile_fk: String = client
        .query_one(
            "SELECT c.conname FROM pg_constraint c
             WHERE c.conname='prefix_acceptance_profile_proposal'
               AND c.conrelid='reborn_prefix_design_acceptances'::regclass
               AND c.confrelid='reborn_prefix_design_proposals'::regclass
               AND c.contype='f'
               AND c.conkey=ARRAY[
                   (SELECT attnum FROM pg_attribute WHERE attrelid=c.conrelid AND attname='proposal_id'),
                   (SELECT attnum FROM pg_attribute WHERE attrelid=c.conrelid AND attname='accepted_profile')
               ]::SMALLINT[]
               AND c.confkey=ARRAY[
                   (SELECT attnum FROM pg_attribute WHERE attrelid=c.confrelid AND attname='proposal_id'),
                   (SELECT attnum FROM pg_attribute WHERE attrelid=c.confrelid AND attname='profile_name')
               ]::SMALLINT[]",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(profile_fk, expected_fks[1]);
    let evidence = "{}";
    let checksum = hex::encode(Sha256::digest(evidence.as_bytes()));
    let error=client.execute("INSERT INTO reborn_prefix_design_acceptances(acceptance_id,proposal_id,candidate_profile,candidate_generation,evidence_bytes,checksum) VALUES($1,$2,$3,$4,$5,$6)",
        &[&Uuid::new_v4(),&id,&candidate.get::<_,String>(0),&candidate.get::<_,String>(1),&evidence,&checksum]).await.unwrap_err();
    let constraint = error.as_db_error().unwrap().constraint().unwrap();
    assert!(expected_fks.contains(&constraint), "{error}");
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM reborn_prefix_design_acceptances WHERE proposal_id=$1",
                &[&id]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    eprintln!(
        "shared pair construction: exact source/template-bound draft, stable retry, unchanged role/limit metadata, forbidden profile acceptance"
    );
}

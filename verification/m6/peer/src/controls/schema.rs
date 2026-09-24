use super::{Control, Host};
use anyhow::{Result, bail, ensure};
use noble_kernel::async_tasks as task;
use serde_json::{Value, json};

pub(super) fn run(case: &str) -> Result<Value> {
    let mut rows =
        Vec::with_capacity(task::STATE_CONSTRUCTORS.len() * task::EVENT_CONSTRUCTORS.len());
    for state in task::STATE_CONSTRUCTORS {
        for event in task::EVENT_CONSTRUCTORS {
            rows.push(task::CoverageRow {
                state,
                event,
                rule: task::classify(state, event.representative()),
            });
        }
    }
    let pairs = rows.iter().map(row_json).collect::<Vec<_>>();
    let mut state_schema = task::STATE_SCHEMA.to_owned();
    let mut event_schema = task::EVENT_SCHEMA.to_owned();
    let expected = match case {
        "lifecycle-matrix" => None,
        "lifecycle-changed-state-schema" => {
            state_schema.push_str("@foreign-revision");
            Some(task::CoverageError::StateSchema)
        }
        "lifecycle-changed-event-schema" => {
            event_schema.push_str("@foreign-revision");
            Some(task::CoverageError::EventSchema)
        }
        "lifecycle-missing-pair" => {
            rows.pop();
            Some(task::CoverageError::MissingPair)
        }
        "lifecycle-duplicate-pair" => {
            rows[1] = rows[0];
            Some(task::CoverageError::DuplicatePair)
        }
        "lifecycle-invalid-pair" => {
            let row = rows
                .iter_mut()
                .find(|row| {
                    row.state == task::State::Pending && row.event == task::EventKind::Deliver
                })
                .unwrap();
            row.rule = Ok(task::Rule::Deliver);
            Some(task::CoverageError::WrongDisposition)
        }
        "lifecycle-invented-state" => {
            state_schema = state_schema.replace("Pending", "InventedPending");
            Some(task::CoverageError::StateSchema)
        }
        "lifecycle-invented-event" => {
            event_schema = event_schema.replace("TakeWake", "InventedEvent");
            Some(task::CoverageError::EventSchema)
        }
        "lifecycle-invented-disposition" => {
            rows[0].rule = Err(task::Error::InvalidRequest);
            Some(task::CoverageError::WrongDisposition)
        }
        _ => bail!("unknown lifecycle schema control {case}"),
    };
    let actual = task::validate_coverage(state_schema.as_bytes(), event_schema.as_bytes(), &rows);
    ensure!(
        actual == expected.map_or(Ok(()), Err),
        "production coverage admission returned {actual:?}"
    );
    let validation = match actual {
        Ok(()) => json!({"status":"accepted"}),
        Err(error) => json!({"status":"rejected","error":format!("{error:?}")}),
    };
    let mut host = Host::new(case)?;
    host.finish_invocation()?;
    let control = Control::new(case);
    let mut result = control.finish(&host);
    result["lifecycle"] = json!({
        "state_schema":task::STATE_SCHEMA,"event_schema":task::EVENT_SCHEMA,
        "states":task::STATE_CONSTRUCTORS.iter().map(|state|format!("{state:?}")).collect::<Vec<_>>(),
        "events":task::EVENT_CONSTRUCTORS.iter().map(|event|format!("{event:?}")).collect::<Vec<_>>(),
        "pairs":pairs,"submitted_pairs":rows.iter().map(row_json).collect::<Vec<_>>(),
        "submitted_state_schema":state_schema,"submitted_event_schema":event_schema,
        "validation":validation,
    });
    Ok(result)
}

fn row_json(row: &task::CoverageRow) -> Value {
    match row.rule {
        Ok(rule) => json!({"state":format!("{:?}",row.state),"event":format!("{:?}",row.event),
            "disposition":"allowed","rule":format!("{rule:?}"),"error":null}),
        Err(error) => json!({"state":format!("{:?}",row.state),"event":format!("{:?}",row.event),
            "disposition":"rejected","rule":null,"error":format!("{error:?}")}),
    }
}

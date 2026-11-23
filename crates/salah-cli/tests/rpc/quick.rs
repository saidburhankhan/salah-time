use super::phase2::good;
use super::{DATE, Fixtures, Harness, NOW, Session};
use serde_json::json;

const TOMORROW: &str = "2031-05-15";

#[test]
fn quick_text_names_today_or_tomorrow_and_item_add_schedules_it() {
    let h = Harness::new();
    let mut rpc = Session::open(&h, NOW);
    let mut fixtures = Fixtures::default();
    let draft = fixtures.check(
        "item_parse_tomorrow",
        rpc.request(
            "item.parse",
            json!({"text":"call mum tomorrow 15m after asr"}),
        ),
    );
    assert_eq!(draft["title"], "Call mum");
    assert_eq!(draft["start_day"], TOMORROW);
    let today = good(&mut rpc, "item.parse", json!({"text":"Today, read daily"}));
    assert_eq!(
        (&today["title"], &today["start_day"]),
        (&json!("Read"), &json!(DATE))
    );
    let plain = good(
        &mut rpc,
        "item.parse",
        json!({"text":"review today's notes"}),
    );
    assert_eq!(plain["title"], "Review today's notes");
    assert!(plain["start_day"].is_null());

    let added = fixtures.check(
        "item_add_quick_tomorrow",
        rpc.request(
            "item.add",
            json!({"op_id":"tomorrow","quick_text":"call mum tomorrow 15m after asr"}),
        ),
    );
    let item = &added["changed"]["items"][0];
    assert_eq!(
        (&item["title"], &item["day"]),
        (&json!("Call mum"), &json!(TOMORROW))
    );
    // An explicit date wins over the parsed day.
    let explicit = good(
        &mut rpc,
        "item.add",
        json!({"op_id":"explicit","quick_text":"pay bill tomorrow","date":"2031-05-20"}),
    );
    assert_eq!(explicit["changed"]["items"][0]["day"], "2031-05-20");

    let titles = |rpc: &mut Session, date: &str| -> Vec<String> {
        let day = good(rpc, "day.get", json!({"date":date}));
        day["blocks"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|b| b["items"].as_array().unwrap().clone())
            .map(|row| row["title"].as_str().unwrap().to_owned())
            .collect()
    };
    assert!(!titles(&mut rpc, DATE).contains(&"Call mum".to_owned()));
    assert!(titles(&mut rpc, TOMORROW).contains(&"Call mum".to_owned()));
}

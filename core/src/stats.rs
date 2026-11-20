//! Library statistics: aggregates computed in SQL so the UI never loads every video.
//! Hidden videos are left out of every figure except the `hidden` counter.

use crate::error::Result;
use crate::models::{GroupStat, MonthCount, Stats, VideoBrief};
use crate::Library;
use rusqlite::params;

/// How many entries the "top" lists hold.
const TOP: usize = 5;

/// Duration buckets, in order: (key, exclusive upper bound in seconds).
const BUCKETS: &[(&str, f64)] = &[("lt5", 300.0), ("5to20", 1200.0), ("20to60", 3600.0), ("1to2h", 7200.0)];

fn bucket_key(duration: Option<f64>) -> &'static str {
    match duration {
        Some(d) if d > 0.0 => BUCKETS.iter().find(|(_, max)| d < *max).map(|(k, _)| *k).unwrap_or("gt2h"),
        _ => "unknown",
    }
}

/// `(year, month)` shifted back by `n` months.
fn months_back(year: i32, month: u32, n: u32) -> (i32, u32) {
    let idx = year * 12 + month as i32 - 1 - n as i32;
    (idx.div_euclid(12), idx.rem_euclid(12) as u32 + 1)
}

impl Library {
    pub fn stats(&self) -> Result<Stats> {
        let c = &self.conn;
        let mut s = Stats::default();

        s.hidden = c.query_row("SELECT COUNT(*) FROM videos WHERE hidden = 1", [], |r| r.get(0))?;
        c.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size), 0), COALESCE(SUM(CASE WHEN duration > 0 THEN duration END), 0),
                    COALESCE(SUM(watched), 0),
                    COALESCE(SUM(CASE WHEN watched = 1 AND duration > 0 THEN duration END), 0),
                    COALESCE(SUM(duration IS NULL), 0),
                    COALESCE(SUM(rating > 0), 0), COALESCE(AVG(CASE WHEN rating > 0 THEN rating END), 0) / 2.0
             FROM videos WHERE hidden = 0",
            [],
            |r| {
                s.videos = r.get(0)?;
                s.total_size = r.get(1)?;
                s.total_duration = r.get(2)?;
                s.watched = r.get(3)?;
                s.watched_duration = r.get(4)?;
                s.unindexed = r.get(5)?;
                s.rated = r.get(6)?;
                s.avg_rating = r.get(7)?;
                Ok(())
            },
        )?;

        // Index 0 = 0.5 stars ... index 9 = 5 stars.
        s.rating_distribution = vec![0; 10];
        {
            let mut st = c.prepare("SELECT rating, COUNT(*) FROM videos WHERE hidden = 0 AND rating BETWEEN 1 AND 10 GROUP BY rating")?;
            for row in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (halves, n) = row?;
                s.rating_distribution[(halves - 1) as usize] = n;
            }
        }

        let mut counts: Vec<(&str, i64)> = BUCKETS.iter().map(|(k, _)| (*k, 0)).collect();
        counts.push(("gt2h", 0));
        counts.push(("unknown", 0));
        {
            let mut st = c.prepare("SELECT duration FROM videos WHERE hidden = 0")?;
            for row in st.query_map([], |r| r.get::<_, Option<f64>>(0))? {
                let key = bucket_key(row?);
                if let Some(e) = counts.iter_mut().find(|(k, _)| *k == key) {
                    e.1 += 1;
                }
            }
        }
        s.duration_buckets = counts.into_iter().map(|(k, n)| (k.to_string(), n)).map(|(key, count)| crate::models::Bucket { key, count }).collect();

        let group = |sql: &str| -> Result<Vec<GroupStat>> {
            let mut st = c.prepare(sql)?;
            let rows = st.query_map([], |r| {
                Ok(GroupStat {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    color: r.get(2)?,
                    videos: r.get(3)?,
                    watched: r.get(4)?,
                    size: r.get(5)?,
                    duration: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        };
        let agg = "COUNT(v.id), COALESCE(SUM(v.watched), 0), COALESCE(SUM(v.size), 0),
                   COALESCE(SUM(CASE WHEN v.duration > 0 THEN v.duration END), 0)";
        s.folders = group(&format!(
            "SELECT f.id, f.path, NULL, {agg} FROM folders f LEFT JOIN videos v ON v.folder_id = f.id AND v.hidden = 0
             GROUP BY f.id ORDER BY COUNT(v.id) DESC, f.path"
        ))?;
        s.tags = group(&format!(
            "SELECT t.id, t.name, t.color, {agg} FROM tags t
             LEFT JOIN video_tags vt ON vt.tag_id = t.id LEFT JOIN videos v ON v.id = vt.video_id AND v.hidden = 0
             GROUP BY t.id ORDER BY COUNT(v.id) DESC, t.name COLLATE NOCASE"
        ))?;
        s.playlists = group(&format!(
            "SELECT p.id, p.name, NULL, {agg} FROM playlists p
             LEFT JOIN playlist_items pi ON pi.playlist_id = p.id LEFT JOIN videos v ON v.id = pi.video_id AND v.hidden = 0
             GROUP BY p.id ORDER BY COUNT(v.id) DESC, p.name COLLATE NOCASE"
        ))?;

        let top = |order: &str, filter: &str| -> Result<Vec<VideoBrief>> {
            let mut st = c.prepare(&format!(
                "SELECT id, title, size, duration, rating FROM videos WHERE hidden = 0 AND {filter}
                 ORDER BY {order}, id LIMIT {TOP}"
            ))?;
            let rows = st.query_map([], |r| {
                Ok(VideoBrief {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    size: r.get(2)?,
                    duration: r.get(3)?,
                    rating: r.get::<_, i64>(4)? as f64 / 2.0,
                })
            })?;
            Ok(rows.collect::<std::result::Result<_, _>>()?)
        };
        s.largest = top("size DESC", "1")?;
        s.longest = top("duration DESC", "duration > 0")?;
        s.top_rated = top("rating DESC, size DESC", "rating > 0")?;

        // Videos first seen per month, last 12 months (oldest first, empty months included).
        let (year, month): (i32, u32) = c.query_row(
            "SELECT CAST(strftime('%Y', 'now') AS INTEGER), CAST(strftime('%m', 'now') AS INTEGER)",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        for n in (0..12).rev() {
            let (y, m) = months_back(year, month, n);
            let key = format!("{y:04}-{m:02}");
            let count: i64 = c.query_row(
                "SELECT COUNT(*) FROM videos WHERE hidden = 0 AND strftime('%Y-%m', added_at, 'unixepoch') = ?1",
                params![key],
                |r| r.get(0),
            )?;
            s.added_by_month.push(MonthCount { month: key, count });
        }
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_roll_over_years() {
        assert_eq!(months_back(2026, 3, 0), (2026, 3));
        assert_eq!(months_back(2026, 3, 2), (2026, 1));
        assert_eq!(months_back(2026, 3, 3), (2025, 12));
        assert_eq!(months_back(2026, 1, 12), (2025, 1));
    }

    #[test]
    fn buckets() {
        assert_eq!(bucket_key(None), "unknown");
        assert_eq!(bucket_key(Some(0.0)), "unknown");
        assert_eq!(bucket_key(Some(299.0)), "lt5");
        assert_eq!(bucket_key(Some(300.0)), "5to20");
        assert_eq!(bucket_key(Some(7199.0)), "1to2h");
        assert_eq!(bucket_key(Some(9000.0)), "gt2h");
    }

    #[test]
    fn stats_aggregate_the_library_and_skip_hidden() {
        let lib = Library::open_in_memory().unwrap();
        lib.conn.execute("INSERT INTO folders(id, path) VALUES (1, '/m')", []).unwrap();
        for (id, dur, watched, rating, hidden) in
            [(1, Some(600.0), 1, 8, 0), (2, Some(4000.0), 0, 0, 0), (3, None, 0, 4, 0), (4, Some(100.0), 1, 10, 1)]
        {
            lib.conn
                .execute(
                    "INSERT INTO videos(id, path, folder_id, title, size, duration, watched, rating, hidden)
                     VALUES (?1, ?2, 1, ?2, 1000, ?3, ?4, ?5, ?6)",
                    params![id, format!("/m/{id}.mp4"), dur, watched, rating, hidden],
                )
                .unwrap();
        }
        let tag = lib.create_tag("x", "#fff").unwrap();
        lib.set_video_tag(1, tag.id, true).unwrap();
        lib.set_video_tag(4, tag.id, true).unwrap();

        let s = lib.stats().unwrap();
        assert_eq!((s.videos, s.hidden, s.watched, s.unindexed, s.rated), (3, 1, 1, 1, 2));
        assert_eq!(s.total_size, 3000);
        assert_eq!(s.total_duration, 4600.0);
        assert_eq!(s.watched_duration, 600.0);
        assert_eq!(s.avg_rating, 3.0); // (4 + 2 stars) / 2
        assert_eq!(s.rating_distribution[7], 1); // 4 stars
        assert_eq!(s.rating_distribution[3], 1); // 2 stars
        assert_eq!(s.rating_distribution[9], 0); // the 5-star one is hidden
        let by: std::collections::HashMap<_, _> = s.duration_buckets.iter().map(|b| (b.key.as_str(), b.count)).collect();
        assert_eq!((by["5to20"], by["1to2h"], by["unknown"]), (1, 1, 1));
        assert_eq!(s.tags[0].videos, 1); // hidden video not counted
        assert_eq!((s.folders[0].videos, s.folders[0].watched), (3, 1));
        assert_eq!(s.longest[0].id, 2);
        assert_eq!(s.top_rated[0].id, 1);
        assert_eq!(s.added_by_month.len(), 12);
        assert_eq!(s.added_by_month.last().unwrap().count, 3);
    }
}

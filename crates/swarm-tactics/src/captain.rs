pub fn select_replacement_captain<T: Copy>(
    candidates: impl IntoIterator<Item = (T, f32)>,
) -> Option<T> {
    candidates
        .into_iter()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| entity)
}

#[cfg(test)]
mod tests {
    use super::select_replacement_captain;

    #[test]
    fn picks_strongest_survivor() {
        let selected = select_replacement_captain([(1usize, 12.0), (2usize, 14.5), (3usize, 11.0)]);
        assert_eq!(selected, Some(2));
    }

    #[test]
    fn returns_none_for_empty_candidates() {
        let selected = select_replacement_captain::<usize>([]);
        assert_eq!(selected, None);
    }

    #[test]
    fn preserves_only_survivor() {
        let selected = select_replacement_captain([(42usize, 6.0)]);
        assert_eq!(selected, Some(42));
    }
}

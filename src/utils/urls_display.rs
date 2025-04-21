use url::Url;

pub struct UrlsDisplay<I>(pub I);

impl<'a, I: Iterator<Item = &'a Url> + Clone> std::fmt::Display for UrlsDisplay<I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ls = f.debug_list();
        self.0.clone().for_each(|x| {
            ls.entry(&x.as_str());
        });
        ls.finish()
    }
}

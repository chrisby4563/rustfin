use anyhow::{Context, Ok, Result};
use jellyfin_api::types::{BaseItemDto, BaseItemDtoQueryResult};

use crate::jellyfin_api::{
    api_client::ApiClient,
    models::{collection_type::CollectionType, user_view::UserView},
};

impl ApiClient {
    pub async fn get_user_views(
        &self,
        start_index: Option<usize>,
        limit: Option<usize>,
    ) -> Result<(Vec<UserView>, usize)> {
        let mut url = self
            .root
            .join(&format!("Users/{}/Views", self.account.id))
            .unwrap();

        {
            let mut query_pairs = url.query_pairs_mut();
            if let Some(start_index) = start_index {
                query_pairs.append_pair("StartIndex", &start_index.to_string());
            }
            if let Some(limit) = limit {
                query_pairs.append_pair("Limit", &limit.to_string());
            }
        }

        let res: BaseItemDtoQueryResult = self.client.get(url).send().await?.json().await?;

        let items = res.items;
        let total_record_count = res
            .total_record_count
            .context("Total record count not returned")?;

        let items: Result<Vec<UserView>, _> =
            items.into_iter().map(|item| item.try_into()).collect();

        Ok((items?, total_record_count as usize))
    }

    pub async fn get_collection_items(
        &self,
        collection: &BaseItemDto,
        query: &CollectionQuery,
        start_index: usize,
        limit: usize,
    ) -> Result<(Vec<BaseItemDto>, usize)> {
        let collection_type = CollectionType::from(collection.collection_type);

        let mut url = self
            .root
            .join(&format!("Users/{}/Items", self.account.id))
            .unwrap();

        url.query_pairs_mut()
            .append_pair("ParentId", &collection.id.unwrap().to_string())
            .append_pair("SortBy", query.sort.sort_by())
            .append_pair(
                "SortOrder",
                if query.descending { "Descending" } else { "Ascending" },
            )
            .append_pair("Recursive", "true")
            .append_pair("StartIndex", &start_index.to_string())
            .append_pair("Limit", &limit.to_string());

        url.query_pairs_mut().append_pair(
            "IncludeItemTypes",
            &collection_type
                .item_type()
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<String>>()
                .join(","),
        );

        if query.unplayed_only {
            url.query_pairs_mut().append_pair("IsPlayed", "false");
        }
        match query.letter {
            // „#“: alles vor A (Zahlen, Zeichen)
            Some('#') => {
                url.query_pairs_mut().append_pair("NameLessThan", "A");
            }
            Some(letter) => {
                url.query_pairs_mut()
                    .append_pair("NameStartsWith", &letter.to_string());
            }
            None => {}
        }
        if let Some(genre) = &query.genre {
            url.query_pairs_mut().append_pair("Genres", genre);
        }

        let res: BaseItemDtoQueryResult = self.client.get(url).send().await?.json().await?;

        let items = res.items;
        let total_record_count = res
            .total_record_count
            .context("Total record count not returned")?;

        Ok((items, total_record_count as usize))
    }
}

/// Sortierung in einer Bibliothek.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CollectionSort {
    #[default]
    Name,
    Year,
    DateAdded,
    Rating,
}

impl CollectionSort {
    pub const ALL: [CollectionSort; 4] = [
        CollectionSort::Name,
        CollectionSort::Year,
        CollectionSort::DateAdded,
        CollectionSort::Rating,
    ];

    fn sort_by(&self) -> &'static str {
        match self {
            CollectionSort::Name => "SortName,ProductionYear",
            CollectionSort::Year => "ProductionYear,PremiereDate,SortName",
            CollectionSort::DateAdded => "DateCreated,SortName",
            CollectionSort::Rating => "CommunityRating,SortName",
        }
    }

    /// Sinnvolle Richtung beim Umschalten: neu/Jahr/Bewertung absteigend.
    pub fn default_descending(&self) -> bool {
        !matches!(self, CollectionSort::Name)
    }
}

/// Filter und Sortierung einer Bibliotheksansicht.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CollectionQuery {
    pub sort: CollectionSort,
    pub descending: bool,
    pub unplayed_only: bool,
    /// Anfangsbuchstabe; `#` = Zahlen/Zeichen.
    pub letter: Option<char>,
    pub genre: Option<String>,
}

impl ApiClient {
    /// Genres, die in einer Bibliothek vorkommen (alphabetisch).
    pub async fn get_collection_genres(&self, collection: &BaseItemDto) -> Result<Vec<String>> {
        let mut url = self.root.join("Genres")?;
        url.query_pairs_mut()
            .append_pair("ParentId", &collection.id.unwrap().to_string())
            .append_pair("UserId", &self.account.id.to_string())
            .append_pair("SortBy", "SortName");
        let res: BaseItemDtoQueryResult = self.client.get(url).send().await?.json().await?;
        Ok(res.items.into_iter().filter_map(|genre| genre.name).collect())
    }
}

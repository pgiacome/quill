//! Premium and Business features, keyed by TDLib's `premiumFeature*` and
//! `businessFeature*` constructors.
//!
//! tdesktop names them `PremiumFeature` (`boxes/premium_preview_box.h`) and
//! opens the preview of one with `ShowPremiumPreviewBox(feature)`. Quill
//! keys every upsell and every preview page by these types instead of by
//! a row title.

use crate::lang::{Key, keys, tr};

/// One `premiumFeature*` constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PremiumFeature {
    IncreasedLimits,
    IncreasedUploadFileSize,
    ImprovedDownloadSpeed,
    VoiceRecognition,
    DisabledAds,
    UniqueReactions,
    UniqueStickers,
    CustomEmoji,
    AdvancedChatManagement,
    ProfileBadge,
    EmojiStatus,
    AnimatedProfilePhoto,
    ForumTopicIcon,
    AppIcons,
    RealTimeChatTranslation,
    UpgradedStories,
    ChatBoost,
    AccentColor,
    BackgroundForBoth,
    SavedMessagesTags,
    MessagePrivacy,
    LastSeenTimes,
    Business,
    MessageEffects,
    Checklists,
    PaidMessages,
    ProtectPrivateChatContent,
    TextComposition,
    RichMessages,
}

/// What Quill knows about one Premium feature.
struct PremiumEntry {
    td_type: &'static str,
    title: Key,
    about: Key,
    /// tdesktop `EntryMap` key (`LookupPremiumRef`), the `/start` ref of
    /// the subscribe button.
    start_ref: Option<&'static str>,
}

const fn entry(
    td_type: &'static str,
    title: Key,
    about: Key,
    start_ref: Option<&'static str>,
) -> PremiumEntry {
    PremiumEntry {
        td_type,
        title,
        about,
        start_ref,
    }
}

impl PremiumFeature {
    /// Every feature, in tdesktop's explainer order.
    pub const ALL: [Self; 29] = [
        Self::IncreasedLimits,
        Self::IncreasedUploadFileSize,
        Self::ImprovedDownloadSpeed,
        Self::VoiceRecognition,
        Self::DisabledAds,
        Self::UniqueReactions,
        Self::UniqueStickers,
        Self::CustomEmoji,
        Self::AdvancedChatManagement,
        Self::ProfileBadge,
        Self::EmojiStatus,
        Self::AnimatedProfilePhoto,
        Self::ForumTopicIcon,
        Self::AppIcons,
        Self::RealTimeChatTranslation,
        Self::UpgradedStories,
        Self::ChatBoost,
        Self::AccentColor,
        Self::BackgroundForBoth,
        Self::SavedMessagesTags,
        Self::MessagePrivacy,
        Self::LastSeenTimes,
        Self::Business,
        Self::MessageEffects,
        Self::Checklists,
        Self::PaidMessages,
        Self::ProtectPrivateChatContent,
        Self::TextComposition,
        Self::RichMessages,
    ];

    fn entry(self) -> PremiumEntry {
        match self {
            Self::IncreasedLimits => entry(
                "premiumFeatureIncreasedLimits",
                keys::Q_MONEY_PREMIUM_FEATURE_INCREASED_LIMITS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_INCREASED_LIMITS_ABOUT,
                Some("double_limits"),
            ),
            Self::IncreasedUploadFileSize => entry(
                "premiumFeatureIncreasedUploadFileSize",
                keys::Q_MONEY_PREMIUM_FEATURE_UPLOAD_SIZE_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_UPLOAD_SIZE_ABOUT,
                Some("more_upload"),
            ),
            Self::ImprovedDownloadSpeed => entry(
                "premiumFeatureImprovedDownloadSpeed",
                keys::Q_MONEY_PREMIUM_FEATURE_DOWNLOAD_SPEED_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_DOWNLOAD_SPEED_ABOUT,
                Some("faster_download"),
            ),
            Self::VoiceRecognition => entry(
                "premiumFeatureVoiceRecognition",
                keys::Q_MONEY_PREMIUM_FEATURE_VOICE_RECOGNITION_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_VOICE_RECOGNITION_ABOUT,
                Some("voice_to_text"),
            ),
            Self::DisabledAds => entry(
                "premiumFeatureDisabledAds",
                keys::Q_MONEY_PREMIUM_FEATURE_NO_ADS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_NO_ADS_ABOUT,
                Some("no_ads"),
            ),
            Self::UniqueReactions => entry(
                "premiumFeatureUniqueReactions",
                keys::Q_MONEY_PREMIUM_FEATURE_UNIQUE_REACTIONS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_UNIQUE_REACTIONS_ABOUT,
                Some("infinite_reactions"),
            ),
            Self::UniqueStickers => entry(
                "premiumFeatureUniqueStickers",
                keys::Q_MONEY_PREMIUM_FEATURE_PREMIUM_STICKERS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_PREMIUM_STICKERS_ABOUT,
                Some("premium_stickers"),
            ),
            Self::CustomEmoji => entry(
                "premiumFeatureCustomEmoji",
                keys::Q_MONEY_PREMIUM_FEATURE_ANIMATED_EMOJI_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_ANIMATED_EMOJI_ABOUT,
                Some("animated_emoji"),
            ),
            Self::AdvancedChatManagement => entry(
                "premiumFeatureAdvancedChatManagement",
                keys::Q_MONEY_PREMIUM_FEATURE_ADVANCED_CHAT_MANAGEMENT_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_ADVANCED_CHAT_MANAGEMENT_ABOUT,
                Some("advanced_chat_management"),
            ),
            Self::ProfileBadge => entry(
                "premiumFeatureProfileBadge",
                keys::Q_MONEY_PREMIUM_FEATURE_PROFILE_BADGE_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_PROFILE_BADGE_ABOUT,
                Some("profile_badge"),
            ),
            Self::EmojiStatus => entry(
                "premiumFeatureEmojiStatus",
                keys::Q_MONEY_PREMIUM_FEATURE_EMOJI_STATUS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_EMOJI_STATUS_ABOUT,
                Some("emoji_status"),
            ),
            Self::AnimatedProfilePhoto => entry(
                "premiumFeatureAnimatedProfilePhoto",
                keys::Q_MONEY_PREMIUM_FEATURE_ANIMATED_PROFILE_PHOTO_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_ANIMATED_PROFILE_PHOTO_ABOUT,
                Some("animated_userpics"),
            ),
            Self::ForumTopicIcon => entry(
                "premiumFeatureForumTopicIcon",
                keys::Q_MONEY_PREMIUM_FEATURE_TOPIC_ICONS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_TOPIC_ICONS_ABOUT,
                None,
            ),
            Self::AppIcons => entry(
                "premiumFeatureAppIcons",
                keys::Q_MONEY_PREMIUM_FEATURE_APP_ICONS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_APP_ICONS_ABOUT,
                None,
            ),
            Self::RealTimeChatTranslation => entry(
                "premiumFeatureRealTimeChatTranslation",
                keys::Q_MONEY_PREMIUM_FEATURE_TRANSLATION_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_TRANSLATION_ABOUT,
                Some("translations"),
            ),
            Self::UpgradedStories => entry(
                "premiumFeatureUpgradedStories",
                keys::Q_MONEY_PREMIUM_FEATURE_STORIES_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_STORIES_ABOUT,
                Some("stories"),
            ),
            Self::ChatBoost => entry(
                "premiumFeatureChatBoost",
                keys::Q_MONEY_PREMIUM_FEATURE_BOOSTS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_BOOSTS_ABOUT,
                None,
            ),
            Self::AccentColor => entry(
                "premiumFeatureAccentColor",
                keys::Q_MONEY_PREMIUM_FEATURE_PEER_COLORS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_PEER_COLORS_ABOUT,
                Some("peer_colors"),
            ),
            Self::BackgroundForBoth => entry(
                "premiumFeatureBackgroundForBoth",
                keys::Q_MONEY_PREMIUM_FEATURE_WALLPAPERS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_WALLPAPERS_ABOUT,
                Some("wallpapers"),
            ),
            Self::SavedMessagesTags => entry(
                "premiumFeatureSavedMessagesTags",
                keys::Q_MONEY_PREMIUM_FEATURE_SAVED_TAGS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_SAVED_TAGS_ABOUT,
                Some("saved_tags"),
            ),
            Self::MessagePrivacy => entry(
                "premiumFeatureMessagePrivacy",
                keys::Q_MONEY_PREMIUM_FEATURE_MESSAGE_PRIVACY_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_MESSAGE_PRIVACY_ABOUT,
                Some("message_privacy"),
            ),
            Self::LastSeenTimes => entry(
                "premiumFeatureLastSeenTimes",
                keys::Q_MONEY_PREMIUM_FEATURE_LAST_SEEN_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_LAST_SEEN_ABOUT,
                Some("last_seen"),
            ),
            Self::Business => entry(
                "premiumFeatureBusiness",
                keys::BUSINESS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_BUSINESS_ABOUT,
                Some("business"),
            ),
            Self::MessageEffects => entry(
                "premiumFeatureMessageEffects",
                keys::Q_MONEY_PREMIUM_FEATURE_MESSAGE_EFFECTS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_MESSAGE_EFFECTS_ABOUT,
                Some("effects"),
            ),
            Self::Checklists => entry(
                "premiumFeatureChecklists",
                keys::Q_MONEY_PREMIUM_FEATURE_CHECKLISTS_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_CHECKLISTS_ABOUT,
                Some("todo"),
            ),
            Self::PaidMessages => entry(
                "premiumFeaturePaidMessages",
                keys::Q_MONEY_PREMIUM_FEATURE_PAID_MESSAGES_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_PAID_MESSAGES_ABOUT,
                None,
            ),
            Self::ProtectPrivateChatContent => entry(
                "premiumFeatureProtectPrivateChatContent",
                keys::Q_MONEY_PREMIUM_FEATURE_PROTECT_PRIVATE_CONTENT_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_PROTECT_PRIVATE_CONTENT_ABOUT,
                Some("no_forwards"),
            ),
            Self::TextComposition => entry(
                "premiumFeatureTextComposition",
                keys::Q_MONEY_PREMIUM_FEATURE_TEXT_COMPOSITION_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_TEXT_COMPOSITION_ABOUT,
                Some("ai_compose"),
            ),
            Self::RichMessages => entry(
                "premiumFeatureRichMessages",
                keys::Q_MONEY_PREMIUM_FEATURE_RICH_MESSAGES_TITLE,
                keys::Q_MONEY_PREMIUM_FEATURE_RICH_MESSAGES_ABOUT,
                Some("rich_formatting"),
            ),
        }
    }

    /// The feature behind a `premiumFeature*` constructor; `None` for one
    /// newer than Quill.
    pub fn from_td(constructor: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.td_type() == constructor)
    }

    pub fn td_type(self) -> &'static str {
        self.entry().td_type
    }

    /// Row and preview title.
    pub fn title(self) -> &'static str {
        tr(self.entry().title)
    }

    /// Row and preview description.
    pub fn about(self) -> &'static str {
        tr(self.entry().about)
    }

    /// tdesktop `LookupPremiumRef`: the `/start` ref the subscribe button
    /// sends from this feature's preview.
    pub fn start_ref(self) -> Option<&'static str> {
        self.entry().start_ref
    }
}

/// One `businessFeature*` constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BusinessFeature {
    Location,
    OpeningHours,
    QuickReplies,
    GreetingMessage,
    AwayMessage,
    AccountLinks,
    StartPage,
    Bots,
    EmojiStatus,
    ChatFolderTags,
    UpgradedStories,
}

impl BusinessFeature {
    pub const ALL: [Self; 11] = [
        Self::Location,
        Self::OpeningHours,
        Self::QuickReplies,
        Self::GreetingMessage,
        Self::AwayMessage,
        Self::AccountLinks,
        Self::StartPage,
        Self::Bots,
        Self::EmojiStatus,
        Self::ChatFolderTags,
        Self::UpgradedStories,
    ];

    /// tdesktop `settings_business.cpp` `FallbackOrder`: the Business
    /// list before `getBusinessFeatures` answers.
    pub const FALLBACK_ORDER: [Self; 9] = [
        Self::GreetingMessage,
        Self::AwayMessage,
        Self::QuickReplies,
        Self::OpeningHours,
        Self::Location,
        Self::AccountLinks,
        Self::StartPage,
        Self::Bots,
        Self::ChatFolderTags,
    ];

    pub fn td_type(self) -> &'static str {
        match self {
            Self::Location => "businessFeatureLocation",
            Self::OpeningHours => "businessFeatureOpeningHours",
            Self::QuickReplies => "businessFeatureQuickReplies",
            Self::GreetingMessage => "businessFeatureGreetingMessage",
            Self::AwayMessage => "businessFeatureAwayMessage",
            Self::AccountLinks => "businessFeatureAccountLinks",
            Self::StartPage => "businessFeatureStartPage",
            Self::Bots => "businessFeatureBots",
            Self::EmojiStatus => "businessFeatureEmojiStatus",
            Self::ChatFolderTags => "businessFeatureChatFolderTags",
            Self::UpgradedStories => "businessFeatureUpgradedStories",
        }
    }

    pub fn from_td(constructor: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|feature| feature.td_type() == constructor)
    }

    /// Whether tdesktop lists the feature on the Business screen and in
    /// its preview (`BusinessFeaturesOrder` knows nine; the emoji status
    /// and stories entries are Premium rows there).
    pub fn listed(self) -> bool {
        Self::FALLBACK_ORDER.contains(&self)
    }

    /// Title and description (`lng_business_subtitle_*` /
    /// `lng_business_about_*`; folder tags use the Premium summary keys).
    pub fn text_keys(self) -> (Key, Key) {
        match self {
            Self::Location => (
                keys::BUSINESS_SUBTITLE_LOCATION,
                keys::BUSINESS_ABOUT_LOCATION,
            ),
            Self::OpeningHours => (
                keys::BUSINESS_SUBTITLE_OPENING_HOURS,
                keys::BUSINESS_ABOUT_OPENING_HOURS,
            ),
            Self::QuickReplies => (
                keys::BUSINESS_SUBTITLE_QUICK_REPLIES,
                keys::BUSINESS_ABOUT_QUICK_REPLIES,
            ),
            Self::GreetingMessage => (
                keys::BUSINESS_SUBTITLE_GREETING_MESSAGES,
                keys::BUSINESS_ABOUT_GREETING_MESSAGES,
            ),
            Self::AwayMessage => (
                keys::BUSINESS_SUBTITLE_AWAY_MESSAGES,
                keys::BUSINESS_ABOUT_AWAY_MESSAGES,
            ),
            Self::AccountLinks => (
                keys::BUSINESS_SUBTITLE_CHAT_LINKS,
                keys::BUSINESS_ABOUT_CHAT_LINKS,
            ),
            Self::StartPage => (
                keys::BUSINESS_SUBTITLE_CHAT_INTRO,
                keys::BUSINESS_ABOUT_CHAT_INTRO,
            ),
            Self::Bots => (
                keys::BUSINESS_SUBTITLE_CHATBOTS,
                keys::BUSINESS_ABOUT_CHATBOTS,
            ),
            Self::ChatFolderTags | Self::EmojiStatus | Self::UpgradedStories => (
                keys::PREMIUM_SUMMARY_SUBTITLE_FILTER_TAGS,
                keys::PREMIUM_SUMMARY_ABOUT_FILTER_TAGS,
            ),
        }
    }
}

/// `businessFeatures`: the features tdesktop lists, in the server's order.
pub fn parse_business_features(value: &serde_json::Value) -> Vec<BusinessFeature> {
    value
        .get("features")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|row| BusinessFeature::from_td(row.get("@type")?.as_str()?))
        .filter(|feature| feature.listed())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{BusinessFeature, PremiumFeature, parse_business_features};
    use serde_json::json;

    #[test]
    fn premium_features_round_trip_their_constructor() {
        for feature in PremiumFeature::ALL {
            assert_eq!(PremiumFeature::from_td(feature.td_type()), Some(feature));
        }
        for feature in BusinessFeature::ALL {
            assert_eq!(BusinessFeature::from_td(feature.td_type()), Some(feature));
        }
        assert_eq!(PremiumFeature::from_td("premiumFeatureNewerThanThis"), None);
    }

    #[test]
    fn start_refs_follow_tdesktop() {
        assert_eq!(
            PremiumFeature::IncreasedLimits.start_ref(),
            Some("double_limits")
        );
        assert_eq!(PremiumFeature::MessageEffects.start_ref(), Some("effects"));
        assert_eq!(PremiumFeature::ForumTopicIcon.start_ref(), None);
    }

    #[test]
    fn business_list_keeps_server_order_and_drops_unlisted_rows() {
        let list = parse_business_features(&json!({"features": [
            {"@type": "businessFeatureChatFolderTags"},
            {"@type": "businessFeatureEmojiStatus"},
            {"@type": "businessFeatureQuickReplies"},
            {"@type": "businessFeatureUpgradedStories"},
            {"@type": "businessFeatureNewerThanThis"},
            {"@type": "businessFeatureBots"}
        ]}));
        assert_eq!(
            list,
            [
                BusinessFeature::ChatFolderTags,
                BusinessFeature::QuickReplies,
                BusinessFeature::Bots
            ]
        );
    }
}

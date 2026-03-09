-- Swapie Backend: Initial Schema Migration
-- MySQL / MariaDB
-- Idempotent: all statements use CREATE TABLE IF NOT EXISTS

-- ─────────────────────────────────────────────────────────────────────────────
-- Independent tables (no foreign key dependencies)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `roles` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `guard_name`  VARCHAR(191) NOT NULL DEFAULT 'web',
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `roles_name_guard_unique` (`name`, `guard_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `permissions` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `guard_name`  VARCHAR(191) NOT NULL DEFAULT 'web',
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `permissions_name_guard_unique` (`name`, `guard_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `role_has_permissions` (
    `permission_id` BIGINT UNSIGNED NOT NULL,
    `role_id`       BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`permission_id`, `role_id`),
    KEY `role_has_permissions_role_id_foreign` (`role_id`),
    CONSTRAINT `role_has_permissions_permission_id_foreign`
        FOREIGN KEY (`permission_id`) REFERENCES `permissions` (`id`) ON DELETE CASCADE,
    CONSTRAINT `role_has_permissions_role_id_foreign`
        FOREIGN KEY (`role_id`) REFERENCES `roles` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `genres` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `slug`        VARCHAR(191) NOT NULL,
    `type`        VARCHAR(50) NOT NULL DEFAULT 'book' COMMENT 'book | board_game',
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `genres_slug_unique` (`slug`),
    KEY `genres_type_index` (`type`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `tags` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `slug`        VARCHAR(191) NOT NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `tags_slug_unique` (`slug`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `lockers` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `provider`    VARCHAR(50)  NOT NULL COMMENT 'inpost | orlen',
    `address`     VARCHAR(500) NOT NULL,
    `city`        VARCHAR(191) NOT NULL,
    `zip_code`    VARCHAR(20)  NOT NULL,
    `latitude`    DECIMAL(10,7) NOT NULL,
    `longitude`   DECIMAL(10,7) NOT NULL,
    `description` TEXT NULL,
    `is_active`   TINYINT(1) NOT NULL DEFAULT 1,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `lockers_name_provider_unique` (`name`, `provider`),
    KEY `lockers_provider_index` (`provider`),
    KEY `lockers_city_index` (`city`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `delivery_options` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `slug`        VARCHAR(191) NOT NULL,
    `name`        VARCHAR(191) NOT NULL,
    `type`        VARCHAR(50)  NULL COMMENT 'locker | courier | personal',
    `provider`    VARCHAR(50)  NULL COMMENT 'inpost | orlen | dpd | null',
    `price`       DECIMAL(10,2) NOT NULL DEFAULT 0.00,
    `is_active`   TINYINT(1)   NOT NULL DEFAULT 1,
    `sort_order`  INT          NOT NULL DEFAULT 0,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `delivery_options_slug_unique` (`slug`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `settings` (
    `id`           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `option_name`  VARCHAR(191) NOT NULL,
    `option_value` LONGTEXT NULL,
    `autoload`     TINYINT(1) NOT NULL DEFAULT 1,
    `created_at`   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `settings_option_name_unique` (`option_name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `taxonomies` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`        VARCHAR(191) NOT NULL,
    `slug`        VARCHAR(191) NOT NULL,
    `description` TEXT NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `taxonomies_slug_unique` (`slug`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `modules` (
    `id`            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `name`          VARCHAR(191) NOT NULL,
    `display_name`  VARCHAR(191) NOT NULL,
    `description`   TEXT NULL,
    `is_active`     TINYINT(1) NOT NULL DEFAULT 1,
    `created_at`    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `modules_name_unique` (`name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Media (referenced by users.avatar_id)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `media` (
    `id`              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `model_type`      VARCHAR(191) NULL,
    `model_id`        BIGINT UNSIGNED NULL,
    `collection_name` VARCHAR(191) NOT NULL DEFAULT 'default',
    `name`            VARCHAR(191) NOT NULL,
    `file_name`       VARCHAR(191) NOT NULL,
    `mime_type`       VARCHAR(191) NULL,
    `disk`            VARCHAR(191) NOT NULL DEFAULT 'local',
    `size`            BIGINT UNSIGNED NOT NULL DEFAULT 0,
    `created_at`      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `media_model_type_model_id_index` (`model_type`, `model_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Users
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `users` (
    `id`                             BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `email`                          VARCHAR(191) NOT NULL,
    `username`                       VARCHAR(191) NOT NULL,
    `password`                       VARCHAR(255) NOT NULL,
    `first_name`                     VARCHAR(191) NOT NULL DEFAULT '',
    `last_name`                      VARCHAR(191) NOT NULL DEFAULT '',
    `description`                    TEXT NULL,
    `phone_number`                   VARCHAR(50) NULL,
    `avatar_id`                      BIGINT UNSIGNED NULL,
    `language`                       VARCHAR(10) NOT NULL DEFAULT 'en',
    `locker_id`                      BIGINT UNSIGNED NULL,
    `stripe_customer_id`             VARCHAR(191) NULL,
    `stripe_connect_account_id`      VARCHAR(191) NULL,
    `stripe_connect_status`          VARCHAR(50)  NULL,
    `stripe_connect_charges_enabled` TINYINT(1) NOT NULL DEFAULT 0,
    `stripe_connect_payouts_enabled` TINYINT(1) NOT NULL DEFAULT 0,
    `stripe_connect_onboarded_at`    DATETIME NULL,
    `privacy_policy_accepted`        TINYINT(1) NOT NULL DEFAULT 0,
    `terms_of_service_accepted`      TINYINT(1) NOT NULL DEFAULT 0,
    `marketing_emails_accepted`      TINYINT(1) NOT NULL DEFAULT 0,
    `consents_accepted_at`           DATETIME NULL,
    `google_id`                      VARCHAR(191) NULL,
    `facebook_id`                    VARCHAR(191) NULL,
    `social_provider`                VARCHAR(50) NULL,
    `social_provider_id`             VARCHAR(191) NULL,
    `average_rating`                 DECIMAL(3,2) NULL,
    `review_count`                   INT NOT NULL DEFAULT 0,
    `activation_code_expires_at`     DATETIME NULL,
    `last_unread_notification_at`    DATETIME NULL,
    `preferred_item_types`           VARCHAR(191) NULL COMMENT 'JSON array: ["book","board_game"]',
    `default_inpost_locker`          VARCHAR(191) NULL,
    `email_verified_at`              DATETIME NULL,
    `created_at`                     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`                     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `users_email_unique` (`email`),
    UNIQUE KEY `users_username_unique` (`username`),
    KEY `users_phone_number_index` (`phone_number`),
    KEY `users_google_id_index` (`google_id`),
    KEY `users_facebook_id_index` (`facebook_id`),
    KEY `users_stripe_customer_id_index` (`stripe_customer_id`),
    KEY `users_stripe_connect_account_id_index` (`stripe_connect_account_id`),
    CONSTRAINT `users_avatar_id_foreign`
        FOREIGN KEY (`avatar_id`) REFERENCES `media` (`id`) ON DELETE SET NULL,
    CONSTRAINT `users_locker_id_foreign`
        FOREIGN KEY (`locker_id`) REFERENCES `lockers` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- User-related junction / detail tables
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `model_has_roles` (
    `role_id`    BIGINT UNSIGNED NOT NULL,
    `model_type` VARCHAR(191)   NOT NULL DEFAULT 'App\\Models\\User',
    `model_id`   BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`role_id`, `model_id`, `model_type`),
    KEY `model_has_roles_model_id_model_type_index` (`model_id`, `model_type`),
    CONSTRAINT `model_has_roles_role_id_foreign`
        FOREIGN KEY (`role_id`) REFERENCES `roles` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `model_has_permissions` (
    `permission_id` BIGINT UNSIGNED NOT NULL,
    `model_type`    VARCHAR(191)   NOT NULL DEFAULT 'App\\Models\\User',
    `model_id`      BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`permission_id`, `model_id`, `model_type`),
    KEY `model_has_permissions_model_id_model_type_index` (`model_id`, `model_type`),
    CONSTRAINT `model_has_permissions_permission_id_foreign`
        FOREIGN KEY (`permission_id`) REFERENCES `permissions` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `user_addresses` (
    `id`              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`         BIGINT UNSIGNED NOT NULL,
    `street`          VARCHAR(255) NOT NULL,
    `building_number` VARCHAR(50)  NOT NULL,
    `flat_number`     VARCHAR(50)  NULL,
    `zip_code`        VARCHAR(20)  NOT NULL,
    `city`            VARCHAR(191) NOT NULL,
    `created_at`      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `user_addresses_user_id_index` (`user_id`),
    CONSTRAINT `user_addresses_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `user_blocks` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `blocker_id`  BIGINT UNSIGNED NOT NULL,
    `blocked_id`  BIGINT UNSIGNED NOT NULL,
    `reason`      TEXT NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `user_blocks_blocker_blocked_unique` (`blocker_id`, `blocked_id`),
    KEY `user_blocks_blocked_id_index` (`blocked_id`),
    CONSTRAINT `user_blocks_blocker_id_foreign`
        FOREIGN KEY (`blocker_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `user_blocks_blocked_id_foreign`
        FOREIGN KEY (`blocked_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `sms_verification_codes` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `phone_number` VARCHAR(50) NOT NULL,
    `code`        VARCHAR(10) NOT NULL,
    `purpose`     VARCHAR(50) NOT NULL DEFAULT 'registration' COMMENT 'registration | login | phone_change',
    `user_id`     BIGINT UNSIGNED NULL,
    `attempts`    INT NOT NULL DEFAULT 0,
    `verified_at` DATETIME NULL,
    `expires_at`  DATETIME NOT NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `sms_codes_phone_index` (`phone_number`),
    KEY `sms_codes_user_id_index` (`user_id`),
    CONSTRAINT `sms_codes_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `personal_access_tokens` (
    `id`             BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `tokenable_type` VARCHAR(191)    NOT NULL,
    `tokenable_id`   BIGINT UNSIGNED NOT NULL,
    `name`           VARCHAR(191)    NOT NULL,
    `token`          VARCHAR(64)     NOT NULL,
    `abilities`      TEXT NULL,
    `last_used_at`   DATETIME NULL,
    `expires_at`     DATETIME NULL,
    `created_at`     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`     DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `personal_access_tokens_token_unique` (`token`),
    KEY `personal_access_tokens_tokenable_index` (`tokenable_type`, `tokenable_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `password_reset_tokens` (
    `email`      VARCHAR(191) NOT NULL,
    `token`      VARCHAR(255) NOT NULL,
    `created_at` DATETIME NULL,
    PRIMARY KEY (`email`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Books
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `books` (
    `id`                BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`           BIGINT UNSIGNED NOT NULL,
    `type`              VARCHAR(50) NOT NULL DEFAULT 'book' COMMENT 'book | board_game',
    `title`             VARCHAR(500) NOT NULL,
    `author`            VARCHAR(255) NULL,
    `isbn`              VARCHAR(50)  NULL,
    `description`       TEXT NULL,
    `condition`         VARCHAR(50) NOT NULL DEFAULT 'used_good' COMMENT 'new | used_very_good | used_good | used_acceptable',
    `status`            VARCHAR(50) NOT NULL DEFAULT 'active' COMMENT 'active | inactive | pending_exchange | sold | matched',
    `for_exchange`      TINYINT(1) NOT NULL DEFAULT 1,
    `for_sale`          TINYINT(1) NOT NULL DEFAULT 0,
    `price`             DECIMAL(10,2) NULL,
    `location`          VARCHAR(255) NULL,
    `latitude`          DECIMAL(10,7) NULL,
    `longitude`         DECIMAL(10,7) NULL,
    `category`          VARCHAR(191) NULL,
    `language`          VARCHAR(50)  NULL,
    `pages_count`       INT NULL,
    `book_format`       VARCHAR(50) NULL COMMENT 'pocket | standard | large',
    `views_count`       INT NOT NULL DEFAULT 0,
    `likes_count`       INT NOT NULL DEFAULT 0,
    `min_players`       INT NULL,
    `max_players`       INT NULL,
    `playing_time`      INT NULL,
    `age_rating`        INT NULL,
    `wanted_isbn`       VARCHAR(50) NULL,
    `wanted_title`      VARCHAR(500) NULL,
    `use_profile_filters` TINYINT(1) NOT NULL DEFAULT 0,
    `created_at`        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`        DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    `deleted_at`        DATETIME NULL,
    KEY `books_user_id_index` (`user_id`),
    KEY `books_status_index` (`status`),
    KEY `books_type_index` (`type`),
    KEY `books_isbn_index` (`isbn`),
    KEY `books_condition_index` (`condition`),
    KEY `books_deleted_at_index` (`deleted_at`),
    FULLTEXT KEY `books_title_author_fulltext` (`title`, `author`),
    CONSTRAINT `books_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `book_images` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `book_id`     BIGINT UNSIGNED NOT NULL,
    `image_path`  VARCHAR(500) NOT NULL,
    `is_primary`  TINYINT(1) NOT NULL DEFAULT 0,
    `order`       INT NOT NULL DEFAULT 0,
    KEY `book_images_book_id_index` (`book_id`),
    CONSTRAINT `book_images_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `book_changes` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `book_id`     BIGINT UNSIGNED NOT NULL,
    `user_id`     BIGINT UNSIGNED NOT NULL,
    `field_name`  VARCHAR(191) NOT NULL,
    `old_value`   TEXT NULL,
    `new_value`   TEXT NULL,
    `ip_address`  VARCHAR(45) NULL,
    `user_agent`  VARCHAR(500) NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY `book_changes_book_id_index` (`book_id`),
    KEY `book_changes_user_id_index` (`user_id`),
    CONSTRAINT `book_changes_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_changes_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Book <-> Genre pivot
CREATE TABLE IF NOT EXISTS `book_genre` (
    `book_id`  BIGINT UNSIGNED NOT NULL,
    `genre_id` BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`book_id`, `genre_id`),
    KEY `book_genre_genre_id_index` (`genre_id`),
    CONSTRAINT `book_genre_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_genre_genre_id_foreign`
        FOREIGN KEY (`genre_id`) REFERENCES `genres` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Book <-> Tag pivot
CREATE TABLE IF NOT EXISTS `book_tag` (
    `book_id` BIGINT UNSIGNED NOT NULL,
    `tag_id`  BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`book_id`, `tag_id`),
    KEY `book_tag_tag_id_index` (`tag_id`),
    CONSTRAINT `book_tag_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_tag_tag_id_foreign`
        FOREIGN KEY (`tag_id`) REFERENCES `tags` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- User preferred genres
CREATE TABLE IF NOT EXISTS `user_genre` (
    `user_id`  BIGINT UNSIGNED NOT NULL,
    `genre_id` BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`user_id`, `genre_id`),
    KEY `user_genre_genre_id_index` (`genre_id`),
    CONSTRAINT `user_genre_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `user_genre_genre_id_foreign`
        FOREIGN KEY (`genre_id`) REFERENCES `genres` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Swipes & Matches
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `swipes` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`     BIGINT UNSIGNED NOT NULL,
    `book_id`     BIGINT UNSIGNED NOT NULL,
    `type`        VARCHAR(50) NOT NULL COMMENT 'like | superlike | reject',
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `swipes_user_book_unique` (`user_id`, `book_id`),
    KEY `swipes_book_id_index` (`book_id`),
    KEY `swipes_type_index` (`type`),
    CONSTRAINT `swipes_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `swipes_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `book_matches` (
    `id`                  BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `book_owner_id`       BIGINT UNSIGNED NOT NULL,
    `interested_user_id`  BIGINT UNSIGNED NOT NULL,
    `owner_book_id`       BIGINT UNSIGNED NOT NULL,
    `interested_book_id`  BIGINT UNSIGNED NULL,
    `type`                VARCHAR(50) NOT NULL DEFAULT 'exchange' COMMENT 'exchange | purchase',
    `status`              VARCHAR(50) NOT NULL DEFAULT 'pending' COMMENT 'pending | accepted | rejected | completed',
    `matched_at`          DATETIME NULL,
    `accepted_at`         DATETIME NULL,
    `completed_at`        DATETIME NULL,
    `created_at`          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `book_matches_book_owner_id_index` (`book_owner_id`),
    KEY `book_matches_interested_user_id_index` (`interested_user_id`),
    KEY `book_matches_owner_book_id_index` (`owner_book_id`),
    KEY `book_matches_interested_book_id_index` (`interested_book_id`),
    KEY `book_matches_status_index` (`status`),
    CONSTRAINT `book_matches_book_owner_id_foreign`
        FOREIGN KEY (`book_owner_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_matches_interested_user_id_foreign`
        FOREIGN KEY (`interested_user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_matches_owner_book_id_foreign`
        FOREIGN KEY (`owner_book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE,
    CONSTRAINT `book_matches_interested_book_id_foreign`
        FOREIGN KEY (`interested_book_id`) REFERENCES `books` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Trades (Offers)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `trades` (
    `id`                                  BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `initiator_id`                        BIGINT UNSIGNED NOT NULL,
    `recipient_id`                        BIGINT UNSIGNED NOT NULL,
    `status`                              VARCHAR(50) NOT NULL DEFAULT 'pending'
        COMMENT 'pending | accepted | rejected | countered | shipped | delivered | completed | disputed | cancelled | awaiting_shipment',
    `cash_top_up`                         DECIMAL(10,2) NULL,
    `top_up_payer`                        VARCHAR(50) NULL COMMENT 'initiator | recipient',
    `protection_fee`                      DECIMAL(10,2) NULL,
    `shipping_cost`                       DECIMAL(10,2) NULL,
    `initiator_shipping_cost`             DECIMAL(10,2) NULL,
    `recipient_shipping_cost`             DECIMAL(10,2) NULL,
    `initiator_delivery_method`           VARCHAR(50) NULL,
    `initiator_locker`                    VARCHAR(191) NULL,
    `recipient_delivery_method`           VARCHAR(50) NULL,
    `recipient_locker`                    VARCHAR(191) NULL,
    `initiator_paid`                      TINYINT(1) NOT NULL DEFAULT 0,
    `recipient_paid`                      TINYINT(1) NOT NULL DEFAULT 0,
    `initiator_paid_amount`               DECIMAL(10,2) NULL,
    `recipient_paid_amount`               DECIMAL(10,2) NULL,
    `initiator_confirmed_delivery`        TINYINT(1) NOT NULL DEFAULT 0,
    `recipient_confirmed_delivery`        TINYINT(1) NOT NULL DEFAULT 0,
    `initiator_confirmed_at`              DATETIME NULL,
    `recipient_confirmed_at`              DATETIME NULL,
    `has_dispute`                         TINYINT(1) NOT NULL DEFAULT 0,
    `dispute_opened_by`                   BIGINT UNSIGNED NULL,
    `dispute_reason`                      TEXT NULL,
    `dispute_opened_at`                   DATETIME NULL,
    `dispute_resolved_at`                 DATETIME NULL,
    `dispute_resolution`                  TEXT NULL,
    `escrow_status`                       VARCHAR(50) NULL COMMENT 'held | released | refunded',
    `escrow_payment_source`               VARCHAR(50) NULL COMMENT 'wallet | stripe',
    `stripe_payment_intent_id`            VARCHAR(191) NULL,
    `stripe_transfer_id`                  VARCHAR(191) NULL,
    `escrow_released_at`                  DATETIME NULL,
    `initiator_to_recipient_shipment_id`  VARCHAR(191) NULL,
    `initiator_to_recipient_label_url`    VARCHAR(500) NULL,
    `recipient_to_initiator_shipment_id`  VARCHAR(191) NULL,
    `recipient_to_initiator_label_url`    VARCHAR(500) NULL,
    `auto_complete_at`                    DATETIME NULL,
    `created_at`                          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`                          DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `trades_initiator_id_index` (`initiator_id`),
    KEY `trades_recipient_id_index` (`recipient_id`),
    KEY `trades_status_index` (`status`),
    KEY `trades_stripe_payment_intent_id_index` (`stripe_payment_intent_id`),
    CONSTRAINT `trades_initiator_id_foreign`
        FOREIGN KEY (`initiator_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `trades_recipient_id_foreign`
        FOREIGN KEY (`recipient_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `trade_items` (
    `id`            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `trade_id`      BIGINT UNSIGNED NOT NULL,
    `book_id`       BIGINT UNSIGNED NOT NULL,
    `owner_id`      BIGINT UNSIGNED NOT NULL,
    `book_snapshot`  JSON NULL COMMENT 'Snapshot of book data at the time of trade creation',
    KEY `trade_items_trade_id_index` (`trade_id`),
    KEY `trade_items_book_id_index` (`book_id`),
    KEY `trade_items_owner_id_index` (`owner_id`),
    CONSTRAINT `trade_items_trade_id_foreign`
        FOREIGN KEY (`trade_id`) REFERENCES `trades` (`id`) ON DELETE CASCADE,
    CONSTRAINT `trade_items_book_id_foreign`
        FOREIGN KEY (`book_id`) REFERENCES `books` (`id`) ON DELETE CASCADE,
    CONSTRAINT `trade_items_owner_id_foreign`
        FOREIGN KEY (`owner_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Messages (Chat)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `messages` (
    `id`               BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `trade_id`         BIGINT UNSIGNED NOT NULL,
    `sender_id`        BIGINT UNSIGNED NULL,
    `content`          TEXT NOT NULL,
    `type`             VARCHAR(50) NULL DEFAULT 'text' COMMENT 'text | image | system | offer_update',
    `status`           VARCHAR(50) NOT NULL DEFAULT 'sent' COMMENT 'pending | sent | delivered | failed',
    `is_read`          TINYINT(1) NOT NULL DEFAULT 0,
    `is_system_message` TINYINT(1) NOT NULL DEFAULT 0,
    `metadata`         JSON NULL,
    `idempotency_key`  VARCHAR(191) NULL,
    `push_sent`        TINYINT(1) NOT NULL DEFAULT 0,
    `push_sent_at`     DATETIME NULL,
    `created_at`       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `messages_idempotency_key_unique` (`idempotency_key`),
    KEY `messages_trade_id_index` (`trade_id`),
    KEY `messages_sender_id_index` (`sender_id`),
    KEY `messages_is_read_index` (`is_read`),
    CONSTRAINT `messages_trade_id_foreign`
        FOREIGN KEY (`trade_id`) REFERENCES `trades` (`id`) ON DELETE CASCADE,
    CONSTRAINT `messages_sender_id_foreign`
        FOREIGN KEY (`sender_id`) REFERENCES `users` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Reviews
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `reviews` (
    `id`               BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `trade_id`         BIGINT UNSIGNED NOT NULL,
    `reviewer_id`      BIGINT UNSIGNED NOT NULL,
    `reviewed_user_id` BIGINT UNSIGNED NOT NULL,
    `rating`           TINYINT NOT NULL COMMENT '1-5',
    `comment`          TEXT NULL,
    `created_at`       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `reviews_trade_reviewer_unique` (`trade_id`, `reviewer_id`),
    KEY `reviews_reviewed_user_id_index` (`reviewed_user_id`),
    KEY `reviews_reviewer_id_index` (`reviewer_id`),
    CONSTRAINT `reviews_trade_id_foreign`
        FOREIGN KEY (`trade_id`) REFERENCES `trades` (`id`) ON DELETE CASCADE,
    CONSTRAINT `reviews_reviewer_id_foreign`
        FOREIGN KEY (`reviewer_id`) REFERENCES `users` (`id`) ON DELETE CASCADE,
    CONSTRAINT `reviews_reviewed_user_id_foreign`
        FOREIGN KEY (`reviewed_user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Payments
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `payment_requests` (
    `id`                         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`                    BIGINT UNSIGNED NOT NULL,
    `external_order_id`          VARCHAR(191) NULL,
    `transaction_id`             VARCHAR(191) NULL,
    `stripe_payment_intent_id`   VARCHAR(191) NULL,
    `stripe_payment_method_id`   VARCHAR(191) NULL,
    `idempotency_key`            VARCHAR(191) NULL,
    `amount`                     DECIMAL(10,2) NOT NULL,
    `method`                     VARCHAR(50)  NULL COMMENT 'stripe | wallet',
    `status`                     VARCHAR(50) NOT NULL DEFAULT 'pending' COMMENT 'pending | processing | completed | failed | expired',
    `error_message`              TEXT NULL,
    `processed_at`               DATETIME NULL,
    `created_at`                 DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`                 DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `payment_requests_idempotency_key_unique` (`idempotency_key`),
    KEY `payment_requests_user_id_index` (`user_id`),
    KEY `payment_requests_status_index` (`status`),
    KEY `payment_requests_stripe_pi_index` (`stripe_payment_intent_id`),
    CONSTRAINT `payment_requests_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `stripe_payment_methods` (
    `id`                        BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`                   BIGINT UNSIGNED NOT NULL,
    `stripe_payment_method_id`  VARCHAR(191) NOT NULL,
    `type`                      VARCHAR(50)  NOT NULL DEFAULT 'card',
    `card_brand`                VARCHAR(50)  NULL,
    `card_last_four`            VARCHAR(4)   NULL,
    `card_exp_month`            INT NULL,
    `card_exp_year`             INT NULL,
    `is_default`                TINYINT(1) NOT NULL DEFAULT 0,
    `created_at`                DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`                DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `spm_stripe_id_unique` (`stripe_payment_method_id`),
    KEY `stripe_payment_methods_user_id_index` (`user_id`),
    CONSTRAINT `stripe_payment_methods_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `wallets` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`     BIGINT UNSIGNED NOT NULL,
    `balance`     DECIMAL(10,2) NOT NULL DEFAULT 0.00,
    `currency`    VARCHAR(10) NOT NULL DEFAULT 'PLN',
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `wallets_user_id_unique` (`user_id`),
    CONSTRAINT `wallets_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `wallet_transactions` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `wallet_id`   BIGINT UNSIGNED NOT NULL,
    `type`        VARCHAR(50) NOT NULL COMMENT 'topup | withdrawal | escrow_hold | escrow_release | escrow_refund | trade_payment',
    `amount`      DECIMAL(10,2) NOT NULL,
    `balance_after` DECIMAL(10,2) NOT NULL,
    `description` VARCHAR(500) NULL,
    `reference_type` VARCHAR(50) NULL COMMENT 'trade | payment_request',
    `reference_id`   BIGINT UNSIGNED NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY `wallet_transactions_wallet_id_index` (`wallet_id`),
    KEY `wallet_transactions_type_index` (`type`),
    CONSTRAINT `wallet_transactions_wallet_id_foreign`
        FOREIGN KEY (`wallet_id`) REFERENCES `wallets` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Notifications & Device Tokens
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `notifications` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`     BIGINT UNSIGNED NOT NULL,
    `title`       VARCHAR(500) NOT NULL,
    `body`        TEXT NOT NULL,
    `type`        VARCHAR(50) NULL COMMENT 'match | trade | message | review | system',
    `data`        JSON NULL,
    `is_read`     TINYINT(1) NOT NULL DEFAULT 0,
    `read_at`     DATETIME NULL,
    `push_sent`   TINYINT(1) NOT NULL DEFAULT 0,
    `push_sent_at` DATETIME NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `notifications_user_id_index` (`user_id`),
    KEY `notifications_is_read_index` (`is_read`),
    KEY `notifications_type_index` (`type`),
    CONSTRAINT `notifications_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `device_tokens` (
    `id`           BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`      BIGINT UNSIGNED NOT NULL,
    `fcm_token`    VARCHAR(500) NOT NULL,
    `device_type`  VARCHAR(20) NOT NULL COMMENT 'ios | android',
    `is_active`    TINYINT(1) NOT NULL DEFAULT 1,
    `last_used_at` DATETIME NULL,
    `created_at`   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `device_tokens_fcm_token_unique` (`fcm_token`),
    KEY `device_tokens_user_id_index` (`user_id`),
    CONSTRAINT `device_tokens_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Admin: Action Logs, Posts, Terms
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `action_logs` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `type_name`   VARCHAR(191) NOT NULL,
    `action_by`   BIGINT UNSIGNED NOT NULL,
    `title`       VARCHAR(500) NOT NULL,
    `data`        JSON NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    KEY `action_logs_type_name_index` (`type_name`),
    KEY `action_logs_action_by_index` (`action_by`),
    CONSTRAINT `action_logs_action_by_foreign`
        FOREIGN KEY (`action_by`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `posts` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `post_type`   VARCHAR(50) NOT NULL DEFAULT 'page' COMMENT 'page | article | faq',
    `title`       VARCHAR(500) NOT NULL,
    `slug`        VARCHAR(500) NOT NULL,
    `content`     LONGTEXT NULL,
    `status`      VARCHAR(50) NOT NULL DEFAULT 'draft' COMMENT 'draft | published | archived',
    `author_id`   BIGINT UNSIGNED NOT NULL,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `posts_slug_unique` (`slug`),
    KEY `posts_post_type_index` (`post_type`),
    KEY `posts_status_index` (`status`),
    KEY `posts_author_id_index` (`author_id`),
    CONSTRAINT `posts_author_id_foreign`
        FOREIGN KEY (`author_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `post_meta` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `post_id`     BIGINT UNSIGNED NOT NULL,
    `meta_key`    VARCHAR(191) NOT NULL,
    `meta_value`  LONGTEXT NULL,
    KEY `post_meta_post_id_index` (`post_id`),
    KEY `post_meta_meta_key_index` (`meta_key`),
    CONSTRAINT `post_meta_post_id_foreign`
        FOREIGN KEY (`post_id`) REFERENCES `posts` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

CREATE TABLE IF NOT EXISTS `terms` (
    `id`          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `taxonomy_id` BIGINT UNSIGNED NOT NULL,
    `name`        VARCHAR(191) NOT NULL,
    `slug`        VARCHAR(191) NOT NULL,
    `description` TEXT NULL,
    `parent_id`   BIGINT UNSIGNED NULL,
    `sort_order`  INT NOT NULL DEFAULT 0,
    `created_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    `updated_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY `terms_taxonomy_slug_unique` (`taxonomy_id`, `slug`),
    KEY `terms_parent_id_index` (`parent_id`),
    CONSTRAINT `terms_taxonomy_id_foreign`
        FOREIGN KEY (`taxonomy_id`) REFERENCES `taxonomies` (`id`) ON DELETE CASCADE,
    CONSTRAINT `terms_parent_id_foreign`
        FOREIGN KEY (`parent_id`) REFERENCES `terms` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Post <-> Term pivot
CREATE TABLE IF NOT EXISTS `post_term` (
    `post_id` BIGINT UNSIGNED NOT NULL,
    `term_id` BIGINT UNSIGNED NOT NULL,
    PRIMARY KEY (`post_id`, `term_id`),
    KEY `post_term_term_id_index` (`term_id`),
    CONSTRAINT `post_term_post_id_foreign`
        FOREIGN KEY (`post_id`) REFERENCES `posts` (`id`) ON DELETE CASCADE,
    CONSTRAINT `post_term_term_id_foreign`
        FOREIGN KEY (`term_id`) REFERENCES `terms` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Stripe webhook event log (idempotency)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `stripe_webhook_events` (
    `id`            BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `event_id`      VARCHAR(191) NOT NULL,
    `event_type`    VARCHAR(191) NOT NULL,
    `payload`       JSON NULL,
    `processed`     TINYINT(1) NOT NULL DEFAULT 0,
    `processed_at`  DATETIME NULL,
    `created_at`    DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY `stripe_webhook_events_event_id_unique` (`event_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- GDPR consent log
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `gdpr_consent_logs` (
    `id`              BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `user_id`         BIGINT UNSIGNED NOT NULL,
    `consent_type`    VARCHAR(50) NOT NULL COMMENT 'privacy_policy | terms_of_service | marketing',
    `accepted`        TINYINT(1) NOT NULL,
    `ip_address`      VARCHAR(45) NULL,
    `user_agent`      VARCHAR(500) NULL,
    `created_at`      DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    KEY `gdpr_consent_logs_user_id_index` (`user_id`),
    CONSTRAINT `gdpr_consent_logs_user_id_foreign`
        FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ─────────────────────────────────────────────────────────────────────────────
-- Failed jobs (background task retry queue, Laravel-compatible)
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS `failed_jobs` (
    `id`         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    `uuid`       VARCHAR(191) NOT NULL,
    `connection` TEXT NOT NULL,
    `queue`      TEXT NOT NULL,
    `payload`    LONGTEXT NOT NULL,
    `exception`  LONGTEXT NOT NULL,
    `failed_at`  DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY `failed_jobs_uuid_unique` (`uuid`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

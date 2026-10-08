#!/bin/bash
if [[ " $* " == *" -h "* ]]; then
	echo "Usage: source setup.sh [-h] [-c]"
	echo "  -h  Show this help message."
	echo "  -c  Clean up all environment variables set by this script (including .env)."
	echo "  -r  Restart docker containers and reset their volumes"
	return 0 2>/dev/null || exit 0
fi

if [[ " $* " == *" -r "* ]]; then
	docker compose down --volumes
	docker compose up -d
	sqlx migrate run --source server/migrations/postgres
	# PGPASSWORD='nimbus' psql -U nimbus -h localhost -p 5432 -f seed.sql
fi

if [[ " $* " == *" -c "* ]]; then
	echo "Cleaning up environment variables."
	unset DATABASE_URL
	unset VALKEY_URL
	unset REDIS_DB
	unset RUST_LOG
	unset ADDRESS
	unset CSRF_HEADER
	unset CORS_ALLOWED_ORIGINS
	unset CORS_ALLOWED_HEADERS
	unset COOKIE_NAME
	unset COOKIE_DOMAIN
	unset GMAIL_ADDRESS
	unset GMAIL_APP_PASSWORD
	unset MAIL_FROM_NAME
	unset FRONTEND_URL
	echo "Cleaned up environment variables."
	docker compose down --volumes
	return 0 2>/dev/null || exit 0
fi

if [[ -e .env ]]; then
	set -a
	source .env
	set +a
	docker compose up -d
	until pg_isready -h localhost -p 5434 -U nimbus -q; do
		echo "Waiting for Postgres..."
		sleep 1
	done
	sqlx migrate run --source server/migrations/postgres
	# PGPASSWORD='nimbus' psql -U nimbus -h localhost -p 5432 -f seed.sql
else
	echo "No .env file found; Run 'cp example.env .env' to create one."
	return 1 2>/dev/null || exit 1
fi

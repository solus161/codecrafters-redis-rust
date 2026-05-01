#!/bin/bash
{
  # ECHO grape
  printf "*2\r\n\$4\r\nECHO\r\n\$6\r\nbanana\r\n"
  
  # PING
  printf "*1\r\n\$4\r\nPI"  # partial
  sleep 0.1
  printf "NG\r\n"           # remaining

  sleep 0.1
  printf "*"
  sleep 0.1
  printf "1"
  sleep 0.1
  printf "\r"
  sleep 0.1
  printf "\n"
  sleep 0.1
  printf "\$"
  sleep 0.1
  printf "4"
  sleep 0.1
  printf "\r"
  sleep 0.1
  printf "\n"
  sleep 0.1
  printf "P"
  sleep 0.1
  printf "I"
  sleep 0.1
  printf "N"
  sleep 0.1
  printf "G"
  sleep 0.1
  printf "\r"
  sleep 0.1
  printf "\n"

  # ECHO "Hello There! General Kenobiii!"
  printf "*2\r\n"
  sleep 0.1
  printf "\$4\r"
  sleep 0.1
  printf "\n"
  sleep 0.1
  printf "E"
  printf "CHO\r\n"
  sleep 0.1
  printf "\$32\r\n"
  sleep 0.1
  printf '"Hello There! '
  sleep 0.1
  printf 'General Kenobiii!"'
  printf "\r\n"

  # SET foo bar
  printf "*3\r\n\$3\r\nSET\r\n\$3\r\nfoo\r\n\$3\r\nbar\r\n"

  # GET foo
  printf "*2\r\n\$3\r\nGET\r\n\$3\r\nfoo\r\n"

  # GET bar
  printf "*2\r\n\$3\r\nGET\r\n\$3\r\nbar\r\n"
  
  # RPUSH list_key "foo" "bar"
  printf "*4\r\n\$5\r\nRPUSH\r\n\$8\r\nlist_key\r\n\$3\r\nfoo\r\n\$3\r\nbar\r\n"
  
  # LRANGE list_key_none 0 1 => empty array
  printf "*4\r\n\$6\r\nLRANGE\r\n\$13\r\nlist_key_none\r\n\$1\r\n0\r\n\$1\r\n0\r\n"
  
  # RPUSH list_key1 a b c d e f
  printf "*8\r\n\$5\r\nRPUSH\r\n\$9\r\nlist_key1\r\n\$1\r\na\r\n\$1\r\nb\r\n"
  printf "\$1\r\nc\r\n\$1\r\nd\r\n\$1\r\ne\r\n\$1\r\nf\r\n"

  # LRANGE list_key1 0 1
  printf "*4\r\n\$6\r\nLRANGE\r\n\$9\r\nlist_key1\r\n\$1\r\n0\r\n\$1\r\n1\r\n"
  
  # LRANGE list_key1 3 5
  printf "*4\r\n\$6\r\nLRANGE\r\n\$9\r\nlist_key1\r\n\$1\r\n3\r\n\$1\r\n5\r\n"

  # LRANGE list_key1 6 8 => empty array
  printf "*4\r\n\$6\r\nLRANGE\r\n\$9\r\nlist_key1\r\n\$1\r\n6\r\n\$1\r\n8\r\n"
  
  # LRANGE list_key1 3 1 => empty array
  printf "*4\r\n\$6\r\nLRANGE\r\n\$9\r\nlist_key1\r\n\$1\r\n3\r\n\$1\r\n1\r\n"

} | nc localhost 6379

#!/bin/bash
{
  # BLPOP expires
  printf "*2\r\n\$4\r\nECHO\r\n\$22\r\nBLPOP tests expiration\r\n"
  printf "*3\r\n\$5\r\nBLPOP\r\n\$9\r\nlist_keyx\r\n\$3\r\n0.5\r\n"
  sleep 1.0

  # BLPOP with expiration, but served
  printf "*2\r\n\$4\r\nECHO\r\n\$33\r\nBLPOP tests expiration but served\r\n"
  printf "*3\r\n\$5\r\nBLPOP\r\n\$9\r\nlist_key3\r\n\$1\r\n3\r\n"
  sleep 1.0
  printf "*3\r\n\$5\r\nRPUSH\r\n\$9\r\nlist_key3\r\n\$1\r\na\r\n"

  # BLPOP served immediately
  printf "*2\r\n\$4\r\nECHO\r\n\$18\r\nBLPOP tests served\r\n"
  printf "*3\r\n\$5\r\nLPUSH\r\n\$9\r\nlist_key3\r\n\$1\r\na\r\n"
  sleep 1.0
  printf "*3\r\n\$5\r\nBLPOP\r\n\$9\r\nlist_key3\r\n\$1\r\n3\r\n"

  # BLPOP served immediately as timeout too short
  printf "*3\r\n\$5\r\nBLPOP\r\n\$9\r\nlist_key3\r\n\$5\r\n0.001\r\n"

  # BLPOP waits forever
  printf "*2\r\n\$4\r\nECHO\r\n\$22\r\nBLPOP tests wait 4ever\r\n"
  printf "*3\r\n\$5\r\nBLPOP\r\n\$5\r\napple\r\n\$1\r\n0\r\n"
  sleep 1.0
  printf "*3\r\n\$5\r\nBLPOP\r\n\$5\r\napple\r\n\$1\r\n0\r\n"
  sleep 1.0
  printf "*3\r\n\$5\r\nRPUSH\r\n\$5\r\napple\r\n\$6\r\nbanana\r\n"
  sleep 2.0
  printf "*3\r\n\$5\r\nRPUSH\r\n\$5\r\napple\r\n\$6\r\ncocain\r\n"
  
  printf "*2\r\n\$4\r\nTYPE\r\n\$5\r\napple\r\n"

} | nc localhost 6379
